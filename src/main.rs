use crossterm::{
    ExecutableCommand,
    cursor::MoveToColumn,
    event::{Event, KeyCode, KeyEvent, KeyModifiers, read},
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};

use std::{
    env,
    io::{self, Write, pipe},
    path::Path,
    process::{Command, Stdio},
};

fn main() {
    init_shell();

    let mut history: Vec<String> = Vec::new();

    loop {
        let commands = read_input(&mut history);

        if commands.is_empty() || commands[0].is_empty() {
            continue;
        }
        println!();

        run_pipeline(&commands);
    }
}

fn init_shell() {
    println!("🥚 Welcome to eggshell! Type 'eggxit' to escape.");
}

fn read_input(history: &mut Vec<String>) -> Vec<Vec<String>> {
    let mut current_dir = env::current_dir().unwrap();
    let home_dir = dirs::home_dir().unwrap();

    if current_dir.starts_with(home_dir.clone()) {
        current_dir = Path::new("~/")
            .join(current_dir.strip_prefix(home_dir).unwrap())
            .to_path_buf();
    }

    let input = read_input_line(
        format!(
            "{}>",
            current_dir
                .iter()
                .map(|os_str| os_str.to_str().unwrap())
                .filter(|s| *s != "/")
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .take(3)
                .map(|s| *s) // Dereference &&str to &str here
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("/")
        )
        .to_string(),
        &history,
    );
    history.push(input.clone());
    input
        .trim()
        .split('|')
        .map(|s| s.trim().split_whitespace().map(str::to_string).collect())
        .collect()
}

pub fn read_input_line(prompt: String, history: &[String]) -> String {
    let mut buffer = Vec::new();
    let mut cursor = 0;

    let mut temp_history: Vec<Vec<char>> = history.iter().map(|s| s.chars().collect()).collect();
    temp_history.push(buffer.clone());

    let mut history_cursor = history.len();

    enable_raw_mode().unwrap();
    print!("{prompt}");
    io::stdout().flush().unwrap();

    loop {
        let event = read().unwrap();

        if let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = event
        {
            let should_break = match (code, modifiers) {
                (KeyCode::Char(c), mods) => {
                    history_cursor = history.len();
                    let ch = if mods.contains(KeyModifiers::SHIFT) {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    };
                    buffer.insert(cursor, ch);
                    cursor += 1;
                    false
                }
                (KeyCode::Backspace, _) => {
                    if cursor > 0 {
                        cursor -= 1;
                        buffer.remove(cursor);
                    }
                    false
                }
                (KeyCode::Left, _) => {
                    if cursor > 0 {
                        cursor -= 1;
                    }
                    false
                }
                (KeyCode::Right, _) => {
                    if cursor < buffer.len() {
                        cursor += 1;
                    }
                    false
                }
                (KeyCode::Up, _) => {
                    if history_cursor > 0 {
                        if history_cursor == history.len() {
                            temp_history[history_cursor] = buffer.clone();
                        }
                        history_cursor -= 1;
                        buffer = temp_history[history_cursor].clone();
                        cursor = buffer.len();
                    }
                    false
                }
                (KeyCode::Down, _) => {
                    if history_cursor < history.len() {
                        history_cursor += 1;
                        buffer = temp_history[history_cursor].clone();
                        cursor = buffer.len();
                    }
                    false
                }
                (KeyCode::Enter | KeyCode::Esc, _) => true,
                _ => false,
            };

            if should_break {
                break;
            }

            // Redraw line
            io::stdout()
                .execute(MoveToColumn(0))
                .unwrap()
                .execute(Clear(ClearType::CurrentLine))
                .unwrap();
            print!("{prompt}{}", buffer.iter().collect::<String>());
            io::stdout().flush().unwrap();

            // Move cursor to correct position
            io::stdout()
                .execute(MoveToColumn((prompt.len() + cursor) as u16))
                .unwrap();
        }
    }

    disable_raw_mode().unwrap();
    buffer.into_iter().collect()
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

    match program.as_str() {
        "exit" | "eggxit" => {
            println!("🥚 Cracking out. Goodbye!");
            std::process::exit(0);
        }

        "cd" => {
            let new_dir = args.get(0).map(String::as_str).unwrap_or("/");
            if let Err(e) = env::set_current_dir(new_dir) {
                return Err(format!("cd: {}: {}", new_dir, e));
            }
            return Ok(Command::new("true").spawn().unwrap());
        }

        "echo" => {
            let message = args.join(" ") + "\n";
            return pipe_and_forward(message, stdout);
        }

        _ => {}
    }

    Command::new(program)
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to run '{}': {}", program, e))
}

fn pipe_and_forward(message: String, stdout: Stdio) -> Result<std::process::Child, String> {
    let (reader, mut writer) = pipe().map_err(|e| format!("pipe error: {}", e))?;

    std::thread::spawn(move || {
        let _ = writer.write_all(message.as_bytes());
        // writer is dropped automatically here
    });

    Command::new("cat")
        .stdin(Stdio::from(reader))
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to forward pipe: {}", e))
}
