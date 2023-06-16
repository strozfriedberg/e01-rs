use libflate::zlib;
use std::convert::TryFrom;
use std::io::Read;
use std::option::Option;
use std::path::{Path, PathBuf};

extern crate kaitai;
use self::kaitai::*;

use crate::generated::ewf_file_header_v1::*;
use crate::generated::ewf_file_header_v2::*;
use crate::generated::ewf_section_descriptor_v1::*;
use crate::generated::ewf_section_descriptor_v2::*;
use crate::generated::ewf_table_header::*;
use crate::generated::ewf_volume::*;
use crate::generated::ewf_volume_smart::*;

use simple_error::SimpleError;

#[derive(Debug)]
pub enum CompressionMethod {
    None = 0,
    Deflate = 1,
    Bzip = 2,
}

impl TryFrom<u16> for CompressionMethod {
    type Error = SimpleError;

    fn try_from(v: u16) -> Result<Self, Self::Error> {
        match v {
            x if x == Self::None as u16 => Ok(Self::None),
            x if x == Self::Deflate as u16 => Ok(Self::Deflate),
            x if x == Self::Bzip as u16 => Ok(Self::Bzip),
            _ => Err(SimpleError::new(format!(
                "Unknown compression method value: {}",
                v
            ))),
        }
    }
}

#[derive(Debug)]
pub struct SegmentFileHeader {
    pub major_version: u8,
    pub minor_version: u8,
    pub compr_method: CompressionMethod,
    pub segment_number: u16,
}

impl SegmentFileHeader {
    pub fn new(io: &BytesReader) -> Result<Self, SimpleError> {
        let first_bytes = io
            .read_bytes(8)
            .map_err(|e| SimpleError::new(format!("read_bytes error: {:?}", e)))?;

        io.seek(0)
            .map_err(|e| SimpleError::new(format!("seek error: {:?}", e)))?;

        // read file header

        if first_bytes == vec![0x45, 0x56, 0x46, 0x09, 0x0d, 0x0a, 0xff, 0x00] // EWF, EWF-E01, EWF-S01
            || first_bytes == vec![0x4c, 0x56, 0x46, 0x09, 0x0d, 0x0a, 0xff, 0x00]
        // EWF-L01
        // V1
        {
            match EwfFileHeaderV1::read_into::<_, EwfFileHeaderV1>(io, None, None) {
                Ok(h) => {
                    return Ok(SegmentFileHeader {
                        major_version: 1,
                        minor_version: 0,
                        compr_method: CompressionMethod::Deflate,
                        segment_number: *h.segment_number(),
                    });
                }
                Err(e) => {
                    return Err(SimpleError::new(format!(
                        "Error while deserializing EwfFileHeaderV1 struct: {:?}",
                        e
                    )));
                }
            }
        } else if first_bytes == vec![0x45, 0x56, 0x46, 0x32, 0x0d, 0x0a, 0x81, 0x00] // EVF2
            || first_bytes == vec![0x4c, 0x45, 0x46, 0x32, 0x0d, 0x0a, 0x81, 0x00]
        // LEF2
        // V2
        {
            match EwfFileHeaderV2::read_into::<_, EwfFileHeaderV2>(io, None, None) {
                Ok(h) => {
                    return Ok(SegmentFileHeader {
                        major_version: *h.major_version(),
                        minor_version: *h.minor_version(),
                        compr_method: (*h.compression_method()).try_into()?,
                        segment_number: *h.segment_number(),
                    });
                }
                Err(e) => {
                    return Err(SimpleError::new(format!(
                        "Error while deserializing EwfFileHeaderV2 struct: {:?}",
                        e
                    )));
                }
            }
        }

        Err(SimpleError::new(format!("invalid segment file")))
    }
}

#[derive(Debug, Default)]
pub struct VolumeSection {
    pub chunk_count: u32,
    pub sector_per_chunk: u32,
    pub bytes_per_sector: u32,
    pub total_sector_count: u64,
}

impl VolumeSection {
    pub fn new(io: &BytesReader, size: u64) -> Result<Self, SimpleError> {
        // read volume section
        if size == 1052 {
            let vol_section =
                EwfVolume::read_into::<_, EwfVolume>(io, None, None).map_err(|e| {
                    SimpleError::new(format!(
                        "Error while deserializing EwfVolume struct: {:?}",
                        e
                    ))
                })?;
            return Ok(VolumeSection {
                chunk_count: *vol_section.number_of_chunks(),
                sector_per_chunk: *vol_section.sector_per_chunk(),
                bytes_per_sector: *vol_section.bytes_per_sector(),
                total_sector_count: *vol_section.number_of_sectors(),
            });
        } else if size == 94 {
            let vol_section = EwfVolumeSmart::read_into::<_, EwfVolumeSmart>(io, None, None)
                .map_err(|e| {
                    SimpleError::new(format!(
                        "Error while deserializing EwfVolumeSmart struct: {:?}",
                        e
                    ))
                })?;
            return Ok(VolumeSection {
                chunk_count: *vol_section.number_of_chunks(),
                sector_per_chunk: *vol_section.sector_per_chunk(),
                bytes_per_sector: *vol_section.bytes_per_sector(),
                total_sector_count: *vol_section.number_of_sectors() as u64,
            });
        }
        Err(SimpleError::new(format!("Unknown volume size: {}", size)))
    }

    pub fn chunk_size(&self) -> usize {
        self.sector_per_chunk as usize * self.bytes_per_sector as usize
    }

    pub fn max_offset(&self) -> usize {
        self.total_sector_count as usize * self.bytes_per_sector as usize
    }
}

#[derive(Debug)]
pub struct E01Reader {
    volume: VolumeSection,
    segments: Vec<Segment>,
}

#[derive(Debug)]
pub struct Segment {
    io: BytesReader,
    header: SegmentFileHeader,
    chunks: Vec<Chunk>,
    end_of_sectors: u64,
}

#[derive(Debug)]
struct Chunk {
    chunk_number: usize,
    data_offset: u32,
    compressed: bool,
}

impl Segment {
    fn read_table(
        io: &BytesReader,
        _size: u64,
        mut chunk_count: usize,
    ) -> Result<Vec<Chunk>, SimpleError> {
        let table_section = EwfTableHeader::read_into::<_, EwfTableHeader>(io, None, None)
            .map_err(|e| {
                SimpleError::new(format!(
                    "Error while deserializing EwfTableHeader struct: {:?}",
                    e
                ))
            })?;
        let mut data_offset: u32;
        let mut chunks: Vec<Chunk> = Vec::new();
        for _ in 0..*table_section.entry_count() {
            let entry = io.read_u4le().map_err(|e| {
                SimpleError::new(format!("BytesReader::read_u4le() failed: {:?}", e))
            })?;
            data_offset = entry & 0x7fffffff;
            data_offset += *table_section.table_base_offset() as u32;
            chunks.push(Chunk {
                chunk_number: chunk_count,
                data_offset,
                compressed: (entry & 0x80000000) > 0,
            });
            chunk_count += 1;
        }
        Ok(chunks)
    }

    pub fn read<T: AsRef<Path>>(
        f: T,
        volume: &mut Option<VolumeSection>,
    ) -> Result<Self, SimpleError> {
        let io = BytesReader::open(f.as_ref()).unwrap();
        let header = SegmentFileHeader::new(&io)?;
        let mut chunks: Vec<Chunk> = Vec::new();
        let mut end_of_sectors = 0;
        let mut current_offset = io.pos();
        while current_offset < io.size() {
            io.seek(current_offset).map_err(|e| {
                SimpleError::new(format!(
                    "Segment file {}, seek to {} failed: {:?}",
                    f.as_ref().to_string_lossy(),
                    current_offset,
                    e
                ))
            })?;

            let section =
                EwfSectionDescriptorV1::read_into::<_, EwfSectionDescriptorV1>(&io, None, None)
                    .map_err(|e| {
                        SimpleError::new(format!(
                    "Segment file: {}, error while deserializing EwfFileHeaderV2 struct: {:?}",
                    f.as_ref().to_string_lossy(),
                    e
                ))
                    })?;

            let section_offset = *section.next_offset() as usize;
            let section_size = if *section.size() > 0x4c
            /* header size */
            {
                *section.size() - 0x4c
            } else {
                0
            };
            let section_type_full = section.type_string();
            let section_type = section_type_full.trim_matches(char::from(0));

            if section_type == "disk" || section_type == "volume" {
                *volume = Some(VolumeSection::new(&io, section_size)?);
            }

            if section_type == "table" {
                chunks.extend(Segment::read_table(&io, section_size, chunks.len())?);
            }

            if section_type == "sectors" {
                end_of_sectors = io.pos() as u64 + section_size;
            }

            if current_offset == section_offset || section_type == "done" {
                break;
            }

            current_offset = section_offset;
        }

        let segment = Segment {
            io,
            header,
            chunks,
            end_of_sectors,
        };

        Ok(segment)
    }

    fn read_chunk(&self, chunk_number: usize) -> Result<Vec<u8>, SimpleError> {
        let chunk_index = chunk_number - self.chunks.first().unwrap().chunk_number;
        let chunk = &self.chunks[chunk_index];
        self.io
            .seek(chunk.data_offset as usize)
            .map_err(|e| SimpleError::new(format!("Seek error: {:?}", e)))?;

        let end_offset = if chunk_index == self.chunks.len() - 1 {
            self.end_of_sectors
        } else {
            self.chunks[chunk_index + 1].data_offset as u64
        };

        let raw_data = self
            .io
            .read_bytes(end_offset as usize - chunk.data_offset as usize)
            .map_err(|e| SimpleError::new(format!("Read error: {:?}", e)))?;

        if !chunk.compressed {
            // skip 4 bytes of checksum
            // TODO: checksum
            return Ok(raw_data[..raw_data.len() - 4].to_vec());
        }

        let mut decoder = zlib::Decoder::new(&raw_data[..])
            .map_err(|e| SimpleError::new(format!("zlib::Decoder failed: {}", e)))?;
        let mut data = Vec::new();
        decoder
            .read_to_end(&mut data)
            .map_err(|e| SimpleError::new(format!("Decompression failed: {}", e)))?;
        Ok(data)
    }
}

impl E01Reader {
    fn find_all_segments(path: &Path) -> Result<Vec<PathBuf>, SimpleError> {
        let filestem = path
            .file_stem()
            .ok_or_else(|| SimpleError::new("Invalid file name"))?
            .to_str()
            .ok_or_else(|| SimpleError::new("Invalid file name"))?;
        let ext_str = path
            .extension()
            .ok_or_else(|| SimpleError::new("Invalid extension"))?
            .to_str()
            .ok_or_else(|| SimpleError::new("Invalid extension"))?;

        if !['E', 'L', 'S'].contains(&ext_str.chars().nth(0).unwrap().to_ascii_uppercase()) {
            return Err(SimpleError::new(format!(
                "Invalid EWF file: {}",
                path.display()
            )));
        }

        let pattern = format!("{}/{}.[ELS]??", path.parent().unwrap().display(), filestem);
        let files = glob::glob(&pattern).map_err(|_| SimpleError::new("Glob error"))?;
        let mut paths: Vec<PathBuf> = files.filter_map(|f| f.ok()).collect();
        paths.sort();
        Ok(paths)
    }

    pub fn open<T: AsRef<Path>>(f: T) -> Result<Self, SimpleError> {
        let mut segments: Vec<Segment> = Vec::new();
        let mut volume_opt: Option<VolumeSection> = None;
        let segments_path = E01Reader::find_all_segments(f.as_ref())?;
        for s in segments_path {
            segments.push(Segment::read(s, &mut volume_opt)?);
        }
        let volume = volume_opt.ok_or(SimpleError::new(format!("Missing volume section")))?;
        let chunks = segments.iter().fold(0, |acc, i| acc + i.chunks.len());
        if chunks != volume.chunk_count as usize {
            return Err(SimpleError::new(format!("Missing some segment file.")));
        }
        Ok(E01Reader { volume, segments })
    }

    pub fn total_size(&self) -> usize {
        self.volume.max_offset()
    }

    fn get_segment(&self, chunk_number: usize) -> Result<&Segment, SimpleError> {
        self.segments
            .iter()
            .find(|s| {
                (s.chunks.first().unwrap().chunk_number..s.chunks.last().unwrap().chunk_number + 1)
                    .contains(&chunk_number)
            })
            .ok_or_else(|| {
                SimpleError::new(format!("Requested chunk number {} is wrong", chunk_number))
            })
    }

    pub fn read_at_offset(&self, mut offset: usize, buf: &mut [u8]) -> Result<usize, SimpleError> {
        let total_size = self.total_size();
        if offset > total_size {
            return Err(SimpleError::new(format!(
                "Requested offset {} is over max offset {}",
                offset, total_size
            )));
        }

        let mut bytes_read = 0;
        while bytes_read < buf.len() && offset < total_size {
            let chunk_number = offset / self.chunk_size();
            debug_assert!(chunk_number < self.volume.chunk_count as usize);
            let mut data = self.get_segment(chunk_number)?.read_chunk(chunk_number)?;

            if chunk_number * self.chunk_size() + data.len() > total_size {
                data = data[..total_size - chunk_number * self.chunk_size()].to_vec();
            }
            let data_offset = (offset % self.chunk_size()) as usize;

            if buf.len() < bytes_read || data.len() < data_offset {
                println!("todo");
            }

            let remaining_size = std::cmp::min(buf.len() - bytes_read, data.len() - data_offset);
            let remaining_buf = &mut buf[bytes_read..bytes_read + remaining_size];
            remaining_buf.copy_from_slice(&data[data_offset..data_offset + remaining_size]);

            debug_assert!(offset + remaining_size <= total_size);

            bytes_read += remaining_size;
            offset += remaining_size;
        }

        Ok(bytes_read)
    }

    pub fn chunk_size(&self) -> usize {
        self.volume.chunk_size()
    }
}
