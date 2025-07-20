use std::{
    io::{self, Write},
    process::{Command, Stdio},
};

fn main() {
    init_shell();

    loop {
        print!("egg> ");
        io::stdout().flush().unwrap();

        let commands = read_input();

        if commands.is_empty() || commands[0].is_empty() {
            continue;
        }

        run_pipeline(&commands);
    }
}

fn init_shell() {
    println!("🥚 Welcome to eggshell! Type 'eggxit' to escape.");
}

fn read_input() -> Vec<Vec<String>> {
    let mut input = String::new();

    if io::stdin().read_line(&mut input).is_err() {
        eprintln!("Shellshock! Couldn't read input.");
        return vec![];
    }

    input
        .trim()
        .split('|')
        .map(|s| s.trim().split_whitespace().map(str::to_string).collect())
        .collect()
}

fn run_pipeline(commands: &[Vec<String>]) {
    if commands.is_empty() {
        return;
    }

    let mut previous_stdout = None;
    let mut children = Vec::new();

    for (i, command) in commands.iter().enumerate() {
        let stdin = match previous_stdout {
            Some(out) => Stdio::from(out),
            None => Stdio::inherit(),
        };

        let stdout = if i == commands.len() - 1 {
            Stdio::inherit()
        } else {
            Stdio::piped()
        };

        match spawn_command_with_io(command, stdin, stdout) {
            Ok(mut child) => {
                let stdout_pipe = child.stdout.take();
                previous_stdout = stdout_pipe;
                children.push(child);
            }
            Err(e) => {
                eprintln!("🥴 Error in pipeline: {}", e);
                return;
            }
        }
    }

    for mut child in children {
        let _ = child.wait();
    }
}

fn spawn_command_with_io(
    command: &[String],
    stdin: Stdio,
    stdout: Stdio,
) -> Result<std::process::Child, String> {
    if command.is_empty() {
        return Err("Empty command".to_string());
    }

    let program = &command[0];
    let args = &command[1..];

    if program == "exit" || program == "eggxit" {
        println!("🥚 Cracking out. Goodbye!");
        std::process::exit(0);
    }

    Command::new(program)
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to run '{}': {}", program, e))
}
