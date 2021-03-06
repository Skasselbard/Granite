import subprocess
import lola
import argparse
import re
from graphviz import Source as dot
import os

def exec_lola(formula):
    net = "net.lola"
    lola.cd_root()
    subprocess.call(["./target/lola/lola-2.0/src/lola", net, formula, "-p"])

def general_deadlock():
    # p_2 marks program termination
    #exec_lola('--formula=AG(EF(p_2 = 1))')
    exec_lola('--formula=EF (DEADLOCK AND (p_2 = 0 AND p_0 = 0))')

def unconditional_deadlock():
    exec_lola('--formula=EF DEADLOCK')

def can_panic():
    # p_0 marks panic or unwind. Implies p_2
    exec_lola('--formula=EF p_0 > 0')

def neighbors(nodes):
    #nodes_regex = [node + "\W" for node in nodes]
    neighbors = []
    # search all given nodes
    for node in nodes:
        matches = []
        # search all lines for the current node
        for line in open('./net.dot', 'r'):
            if re.search(node + "\W", line):
                matches.append(line)
        # search for the arc definitions in the previously matched lines
        for match in matches:
            if re.search("->", match):
                match = match.replace("\n", "")
                match = match.replace(";", "")
                match = match.replace(node, "")
                match = match.replace("->", "")
                match = match.replace(" ", "")
                neighbors.append(match)
    # match all lines that contain nodes and neighbors
    lines = ["digraph petrinet {"]
    # join initial nodes and neighbors saparated by a '|'. Also add a \W to every element
    regex = "|".join([e + "\W" for e in nodes + neighbors])
    for line in open('./net.dot', 'r'):
        if re.search(regex, line):
            lines.append(line)
    lines.append("}")
    print("".join(lines))
    file=open('neighbors.dot','w')
    file.writelines(lines)
    file.close()
    visualize('neighbors.dot')

def visualize(file):
    dot.from_file(file).render(view=True, format="png")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="analyze the output generated by granite")
    parser.add_argument('-d', '--deadlock',action="store_true", help="Default search for deadlocks. Successful and unsuccessful termination is not concidered as a deadlock")
    parser.add_argument('-u', '--unconditional-deadlock',action="store_true", help="Search for every deadlock. Even program termination is concidered a deadlock")
    parser.add_argument('-p', '--panic',action="store_true", help="Check if it is possible to reach a panic state")
    parser.add_argument('-n', '--neighbors', nargs="*", help="Generates a subnet with the given nodes and all its neighbors and visualizes it.")
    parser.add_argument('-v', '--visualize', action="store_true", help="visualize the graph in graphviz. Doesn't terminate (in time) for larger graphs")

    args = parser.parse_args()

    if args.deadlock:
        general_deadlock()
    if args.unconditional_deadlock:
        unconditional_deadlock()
    if args.panic:
        can_panic()
    if args.neighbors:
        neighbors(args.neighbors)
    if args.visualize:
        visualize("net.dot")