from subprocess import run
from argparse import ArgumentParser
from pathlib import Path

from rich.text import Text
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

def main():
    a = ArgumentParser(description="All currently existing opcodes are tested (uh that's a lie besides unsigned ops). If you do your own just reference the folder.")
    a.add_argument("-p", "--path", default="./vm.exe", help="Path to your compiled VM binary.")
    a.add_argument("-d", "--dir", default="tests", help="The directory containing all tests written by test.py (or your own test writer).")
    a.add_argument("-c", "--cmd", default="", help="Command to prepend (this helper was made for valgrind, so ex. 'valgrind --leak-check=full').")
    a.add_argument("-v", "--verbose", action="store_true", help="Pipes output from the vm (or your specified wrapper using --cmd) to sysout.")

    # setup
    args = a.parse_args()
    console = Console()
    tests = sorted(Path(args.dir).glob("*.stk"))
    
    # console print returns none
    if not tests: return console.print("[red]No tests found![/]")

    passed, failed, results = [], [], []
    console.print(f"[yellow]Running {len(tests)} tests...[/]")
    valgrind = args.cmd and "valgrind" in args.cmd
    for test in tests:
        cmd = args.cmd.split() if args.cmd else []
        result = run(cmd + [args.path, str(test)], capture_output=not args.verbose, text=True)
        
        # valgrind returns 0 or 1 instead
        if valgrind: code = result.returncode in (0, 1)
        else: code = result.returncode == 0
        
        if code:
            passed.append(test.name)
            console.print(f"[green]✓[/] {test.name}")
            results.append((test.name, "[green]PASS[/]", str(result.returncode)))

        else:
            failed.append((test.name, result.returncode, result.stdout, result.stderr))
            console.print(f"[red]✗[/] {test.name} (exit {result.returncode})")
            results.append((test.name, "[red]FAIL[/]", str(result.returncode)))

    # build nice table
    table = Table(title="Results:")
    table.add_column("Name", style="cyan")
    table.add_column("Pass/Fail", justify="center")
    table.add_column("Code", justify="center")
    for name, status, code in results: table.add_row(name, status, code)
    table.add_row(f"[bold]Total: {len(tests)}[/]", f"[green]Passed: {len(passed)}[/]", f"[red]Failed: {len(failed)}[/]")

    # print and (maybe) congratulate
    console.print(table)
    if not failed: return console.print("[bold green]All tests passed![/]")
    
    # log any fails and their output
    console.print(f"\n[bold red]{'='*50}[/]\n")
    for name, code, stdout, stderr in failed:
        console.print(Panel(f"[bold]{name}[/] (code: {code})", style="red", expand=False))
        if stdout: 
            console.print(f"[yellow]stdout:[/]")
            console.print(Text.from_ansi(stdout))
            
        if stderr: 
            console.print(f"[bold red]stderr:[/]")
            console.print(Text.from_ansi(stderr))

if __name__ == "__main__":
    main()