import subprocess

commands = [
    ["cargo", "test", "-p", "arkst-core", "--test", "quarkdown_v260_slides", "--locked"],
]

for command in commands:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, check=True)
