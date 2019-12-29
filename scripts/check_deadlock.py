import subprocess
import lola


def main():
    net = "net.lola"
    formula = '--formula=EF (DEADLOCK AND p_0 = 0 AND p_2 = 0)'
    lola.cd_root()
    subprocess.call(["./target/lola/lola-2.0/src/lola", net, formula, "-p"])


if __name__ == "__main__":
    main()