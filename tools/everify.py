import os, sys, re, argparse, subprocess, glob

def verify_file(path, verbose):
  ewf = subprocess.run(["ewfverify.exe", "-q", path], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)

  ewf_returncode = ewf.returncode
  ewf_stdout_s = ewf.stdout.decode()
  ewf_md5 = re.findall("(?<=\s)[a-f0-9]{32}(?=\s)", ewf_stdout_s)
  ewf_sha1 = re.findall("(?<=\s)[0-9a-f]{40}(?=\s)", ewf_stdout_s)

  #print(ewf.stdout)
  #print(f"ewf_md5: {ewf_md5}")
  #print(f"ewf_sha1: {ewf_sha1}")

  e01 = subprocess.run(["e01verify.exe", path], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)

  e01_returncode = e01.returncode
  e01_stdout_s = e01.stdout.decode()
  e01_md5 = re.findall("(?<=\s)[a-f0-9]{32}(?=\s)", e01_stdout_s)
  e01_sha1 = re.findall("(?<=\s)[0-9a-f]{40}(?=\s)", e01_stdout_s)

  #print(e01.stdout)
  #print(f"e01_md5: {e01_md5}")
  #print(f"e01_sha1: {e01_sha1}")

  failed = False
  msg = ""
  if ewf_returncode != e01_returncode:
    failed = True
    msg += f"Return code doesn't equal: {ewf_returncode} (ewfverify), {e01_returncode} (e01verify)\n"
  if ewf_md5[0] != e01_md5[0]:
    failed = True    
    msg += f"MD5 hashes stored in file doesn't equal: {ewf_md5[0]} (ewfverify), {e01_md5[0]} (e01verify)\n"
  if ewf_md5[1] != e01_md5[1]:
    failed = True
    msg += f"MD5 hash calculated over data doesn't equal: {ewf_md5[1]} (ewfverify), {e01_md5[1]} (e01verify)\n"

  if len(ewf_sha1) > 0:
    if ewf_sha1[0] != e01_sha1[0]:
      failed = True
      msg += f"SHA1 hashes stored in file doesn't equal: {ewf_sha1[0]} (ewfverify), {e01_sha1[0]} (e01verify)\n"
    if ewf_sha1[1] != e01_sha1[1]:
      failed = True
      msg += f"SHA1 hash calculated over data doesn't equal: {ewf_sha1[1]} (ewfverify), {e01_sha1[1]} (e01verify)\n"

  if failed or verbose:
    print(f"{path}")
  if failed:
    print(f"{msg}", end="")

  return failed

parser = argparse.ArgumentParser()
parser.add_argument("path", help="path to an E01 file or dir")
parser.add_argument("-v", "--verbose", action='store_true', help="verbose output")
args = parser.parse_args()

if os.path.isdir(args.path):
  matching_files = glob.glob(args.path + '/*.[Ee]01')
  for f in matching_files:
    verify_file(f, args.verbose)
else:
  verify_file(args.path, args.verbose)
  
sys.exit(0)

