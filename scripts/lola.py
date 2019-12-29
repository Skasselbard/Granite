#!/bin/python3

import os
import sys
import os.path
import tarfile
import requests
import subprocess
#https://stackoverflow.com/questions/16694907/download-large-file-in-python-with-requests/16696317#16696317
def download_file(url, path):
    local_filename = path
    # NOTE the stream=True parameter below
    with requests.get(url, stream=True) as r:
        r.raise_for_status()
        with open(local_filename, 'wb') as f:
            for chunk in r.iter_content(chunk_size=8192): 
                if chunk: # filter out keep-alive new chunks
                    f.write(chunk)
                    # f.flush()
    return local_filename

# Search cargo toml to root the project
# https://stackoverflow.com/questions/37427683/python-search-for-a-file-in-current-directory-and-all-its-parents
def cd_root():
    file_name = "Cargo.toml"
    cur_dir = os.getcwd()
    while True:
        file_list = os.listdir(cur_dir)
        parent_dir = os.path.dirname(cur_dir)
        if file_name in file_list:
            os.chdir(cur_dir)
            break
        else:
            if cur_dir == parent_dir: #if dir is root dir
                print("Cannot find project root")
                sys.exit()
                break
            else:
                cur_dir = parent_dir

def main():
    cd_root()
    # download lola to target directory
    lola = "http://service-technology.org/files/lola/lola-2.0.tar.gz"
    lola_path = "./target/lola"
    try:
        os.mkdir(lola_path)
    except FileExistsError:
        pass
    target = lola_path+"/lola-2.0.tar.gz"
    download_file(lola, target)
    tar = tarfile.open(target)
    tar.extractall(lola_path)
    tar.close()
    os.remove(target)

    os.chdir(lola_path+"/lola-2.0")
    subprocess.call([os.getcwd() + "/configure"]) 
    subprocess.call(["make"])


if __name__ == "__main__":
    main()