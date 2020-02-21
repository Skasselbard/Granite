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
    # parser.add_argument('-u', '--unconditional-deadlock',action="store_true", help="Search for every deadlock. Even program termination is concidered a deadlock")
    # parser.add_argument('-p', '--panic',action="store_true", help="Check if it is possible to reach a panic state")
    # parser.add_argument('-n', '--neighbors', nargs="*", help="Generates a subnet with the given nodes and all its neighbors and visualizes it.")
    # parser.add_argument('-v', '--visualize', action="store_true", help="visualize the graph in graphviz. Doesn't terminate (in time) for larger graphs")

    args = parser.parse_args()

    if args.run:
        run(args.run)
    # if args.unconditional_deadlock:
    #     unconditional_deadlock()
    # if args.panic:
    #     can_panic()
    # if args.neighbors:
    #     neighbors(args.neighbors)
    # if args.visualize:
    #     visualize("net.dot")