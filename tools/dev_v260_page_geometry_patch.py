import subprocess

commands = [
    ["cargo", "test", "-p", "arkst-markdown", "--test", "quarkdown_explicit_pagebreak", "--locked"],
    ["cargo", "test", "-p", "arkst-core", "--test", "quarkdown_explicit_pagebreak", "--locked"],
    ["cargo", "test", "-p", "arkst-core", "--test", "quarkdown_v260_slides", "--locked"],
    ["cargo", "test", "-p", "arkst-typst", "--test", "slides_document_prelude", "--locked"],
    ["cargo", "test", "-p", "arkst-typst-inprocess", "--test", "slides_pdf_contract", "--locked"],
]

for command in commands:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, check=True)
