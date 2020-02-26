import subprocess
import argparse
import os

def run(filename):
    sysroot = subprocess.getoutput("rustc --print sysroot")
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = sysroot + "/lib"
    subprocess.call([
        os.getcwd() + "/target/debug/granite", 
        os.getcwd() + "/tests/sample_programs/" + filename,
        "--",
        "--mir_dump",  
        "--format", 
        "pnml", 
        "lola", 
        "dot"
        ], env=env) 

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Script to run granite with the correct configuration")
    parser.add_argument('-r', '--run', help="run granite on a file from tests/sample_programs")

    args = parser.parse_args()

    if args.run:
        run(args.run)