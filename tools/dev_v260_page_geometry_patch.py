import subprocess

commands = [
    [
        "cargo",
        "test",
        "-p",
        "arkst-core",
        "--test",
        "quarkdown_v260_page_alignment",
        "failed_outer_alignment_conversion_rolls_back_nested_document_state_writes",
        "--locked",
        "--",
        "--nocapture",
    ],
    [
        "cargo",
        "test",
        "-p",
        "arkst-core",
        "--test",
        "quarkdown_v260_page_geometry",
        "failed_geometry_conversion_rolls_back_nested_document_state_writes",
        "--locked",
        "--",
        "--nocapture",
    ],
]

for command in commands:
    print("+", " ".join(command), flush=True)
    completed = subprocess.run(command)
    print("exit:", completed.returncode, flush=True)
