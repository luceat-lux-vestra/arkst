import subprocess

commands = [
    ["cargo", "test", "-p", "arkst-core", "--test", "quarkdown_v260_page_geometry", "--locked"],
    ["cargo", "test", "-p", "arkst-core", "--test", "quarkdown_v260_page_alignment", "--locked"],
    ["cargo", "test", "-p", "arkst-typst-inprocess", "--test", "slides_pdf_contract", "--locked"],
    ["cargo", "test", "-p", "arkst-typst-inprocess", "--test", "auto_page_break", "--locked"],
]

for command in commands:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, check=True)
