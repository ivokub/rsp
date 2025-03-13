#!/usr/bin/env python3
import subprocess
import os
import sys
from pathlib import Path
import argparse

def run_benchmark(executable, args, env_vars, stdout_path, stderr_path):
    env = os.environ.copy()
    env.update(env_vars)
    
    with open(stdout_path, 'w') as stdout_file, open(stderr_path, 'w') as stderr_file:
        process = subprocess.Popen([executable] + args, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        
        for stdout_line in iter(process.stdout.readline, ''):
            sys.stdout.write(stdout_line)
            stdout_file.write(stdout_line)
        
        for stderr_line in iter(process.stderr.readline, ''):
            sys.stderr.write(stderr_line)
            stderr_file.write(stderr_line)
        
        process.stdout.close()
        process.stderr.close()
        process.wait()

def main():
    parser = argparse.ArgumentParser(description="Run benchmarks for executables.")
    parser.add_argument("--chain-id", type=int, default=1, help="Chain ID to run the benchmarks for (default 1).")
    parser.add_argument("--cache-dir", type=str, required=True, help="Path to the cache directory.")
    parser.add_argument("--report-dir", type=str, required=True, help="Path to the report directory.")
    parser.add_argument("--number-of-runs", type=int, default=1, help="Number of runs for each block.")
    parser.add_argument("--limit-block-count", type=int, help="Limit the number of blocks to run.")
    parser.add_argument("--optional-arg", type=str, help="Optional argument to pass to the executable.")
    parser.add_argument("executables", nargs='+', help="List of executables to run.")
    args = parser.parse_args()

    num_runs = args.number_of_runs
    limit_block_count = args.limit_block_count
    cache_dir = Path(args.cache_dir)
    report_dir = Path(args.report_dir)
    executables = args.executables
    optional_arg = args.optional_arg

    env_vars = {
        'CUDA_VISIBLE_DEVICES': '0', # use only first device for now
        'SP1_PROVER': 'cuda', # use cuda backend for SP1
    }
    input_dir = cache_dir / 'input/' / str(args.chain_id)

    block_numbers = sorted([p.stem for p in input_dir.glob('*.bin')])
    if limit_block_count:
        block_numbers = block_numbers[:limit_block_count]

    for run_nr in range(num_runs):
        for block_number in block_numbers:
            for executable in executables:
                executable_name = Path(executable).name
                stdout_path = report_dir / f'{executable_name}/{block_number}-{run_nr}-stdout'
                stderr_path = report_dir / f'{executable_name}/{block_number}-{run_nr}-stderr'
                done_path = report_dir / f'{executable_name}/{block_number}-{run_nr}-done'
                
                if done_path.exists():
                    print(f"Skipping {executable} (Block {block_number}, Run {run_nr}) as it is already done.")
                    continue
                
                args_list = [
                    '--chain-id', args.chain_id,
                    '--cache-dir', str(cache_dir),
                    '--block-number', block_number,
                    '--report-path', f'{report_dir}/{executable_name}/{block_number}-{run_nr}-report.csv',
                    '--prove'
                ]

                if optional_arg:
                    args_list.append(optional_arg)

                stdout_path.parent.mkdir(parents=True, exist_ok=True)
                
                run_benchmark(executable, args_list, env_vars, stdout_path, stderr_path)
                
                # Create the done file to indicate completion
                done_path.touch()

if __name__ == "__main__":
    main()
