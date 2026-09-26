use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use sysinfo::{Disks, Pid, ProcessesToUpdate, System};
use nvml_wrapper::Nvml;

use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_UP, VK_DOWN, VK_RETURN};

const R: &str = "\u{001b}[0m";
const RED: &str = "\u{001b}[48;2;255;0;0m";
const BLUE: &str = "\u{001b}[48;2;0;102;204m";
const AMBER: &str = "\u{001b}[48;2;245;166;35m";
const GREEN: &str = "\u{001b}[48;2;34;139;94m";
const DARED: &str = "\u{001b}[48;2;255;40;40m";

const FRED: &str = "\u{001b}[38;2;255;0;0m";
const FAMBER: &str = "\u{001b}[38;2;245;166;35m";
const FGREEN: &str = "\u{001b}[38;2;34;139;94m";
const FDARED: &str = "\u{001b}[38;2;255;40;40m";

const PURP: &str = "\x1b[38;2;240;110;240m";
const CYAN: &str = "\x1b[38;2;93;156;236m";
const GREENT: &str = "\x1b[38;2;0;128;0m";
const HIGHLIGHT: &str = "\u{001b}[7m";

const WAIT_TIME: u64 = 1000;
const POLL_STEP: u64 = 30;

const GIGABYTE: f64 = 1_073_741_824.0;
const I_GIGABYTE: u64 = 1_073_741_824;

const LEFT_WIDTH: usize = 42;
const RIGHT_WIDTH: usize = 34;
const MARGIN: &str = "              ";
const PROCESS_ROW_COUNT: i32 = 20;

const VK_Q: i32 = 0x51;

fn vis_len(s: &str) -> usize {
    let mut len = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            while let Some(nc) = chars.next() {
                if nc == 'm' { break; }
            }
        } else {
            len += 1;
        }
    }
    len
}

fn pad(s: String, width: usize) -> String {
    let len = vis_len(&s);
    if len < width {
        s + &" ".repeat(width - len)
    } else {
        s
    }
}

fn label_dashes(title: &str, color: &str, total: usize) -> String {
    let label = format!("─── {}{}{} ", color, title, R);
    let visible = 4 + title.chars().count() + 1;
    let dashes = total.saturating_sub(visible);
    format!("{}{}", label, "─".repeat(dashes))
}

struct Bar {
    label: String,
    value: f32,
}

impl Bar {
    fn new(label_: String) -> Bar {
        Bar { label: label_, value: 0.0 }
    }

    fn putbar(&self) -> String {
        let mut bar = format!("{}  [", self.label);

        if self.value == 0.0 { bar = bar + "                    "; }
        if self.value > 0.0 && self.value < 5.0 { bar = bar + &format!("{}#{}                   ", BLUE, R); }
        if self.value >= 5.0 && self.value < 10.0 { bar = bar + &format!("{}##{}                  ", BLUE, R); }
        if self.value >= 10.0 && self.value < 15.0 { bar = bar + &format!("{}###{}                 ", GREEN, R); }
        if self.value >= 15.0 && self.value < 20.0 { bar = bar + &format!("{}####{}                ", GREEN, R); }
        if self.value >= 20.0 && self.value < 25.0 { bar = bar + &format!("{}#####{}               ", GREEN, R); }
        if self.value >= 25.0 && self.value < 30.0 { bar = bar + &format!("{}######{}              ", GREEN, R); }
        if self.value >= 30.0 && self.value < 35.0 { bar = bar + &format!("{}#######{}             ", GREEN, R); }
        if self.value >= 35.0 && self.value < 40.0 { bar = bar + &format!("{}########{}            ", GREEN, R); }
        if self.value >= 40.0 && self.value < 45.0 { bar = bar + &format!("{}#########{}           ", AMBER, R); }
        if self.value >= 45.0 && self.value < 50.0 { bar = bar + &format!("{}##########{}          ", AMBER, R); }
        if self.value >= 50.0 && self.value < 55.0 { bar = bar + &format!("{}###########{}         ", AMBER, R); }
        if self.value >= 55.0 && self.value < 60.0 { bar = bar + &format!("{}############{}        ", AMBER, R); }
        if self.value >= 60.0 && self.value < 65.0 { bar = bar + &format!("{}#############{}       ", AMBER, R); }
        if self.value >= 65.0 && self.value < 70.0 { bar = bar + &format!("{}##############{}      ", RED, R); }
        if self.value >= 70.0 && self.value < 75.0 { bar = bar + &format!("{}###############{}     ", RED, R); }
        if self.value >= 75.0 && self.value < 80.0 { bar = bar + &format!("{}################{}    ", RED, R); }
        if self.value >= 80.0 && self.value < 85.0 { bar = bar + &format!("{}#################{}   ", RED, R); }
        if self.value >= 85.0 && self.value < 90.0 { bar = bar + &format!("{}##################{}  ", RED, R); }
        if self.value >= 90.0 && self.value < 95.0 { bar = bar + &format!("{}###################{} ", RED, R); }
        if self.value >= 95.0 && self.value < 100.0 { bar = bar + &format!("{}####################{}", RED, R); }
        if self.value == 100.0 { bar = bar + &format!("{}####################{}", DARED, R); }

        let string_value = format!("{:.2}", self.value);

        bar = bar + "]";
        bar = bar + &string_value + "%";

        format!("       {}", bar)
    }
}

struct Fraction {
    label: String,
    a_val: u64,
    b_val: u64,
    separator: char,
    metric: String,
}

impl Fraction {
    fn new(label: String, first_value: u64, second_value: u64, separator: char, met: String) -> Fraction {
        Fraction { label, a_val: first_value, b_val: second_value, separator, metric: met }
    }

    fn putfrac(&self) -> String {
        let col = if self.b_val == 0 {
            FGREEN
        } else if self.a_val * 3 < self.b_val {
            FGREEN
        } else if self.a_val * 3 < self.b_val * 2 {
            FAMBER
        } else if self.a_val * 10 < self.b_val * 9 {
            FRED
        } else {
            FDARED
        };

        format!("       {}: {}{}{} {} {} {}", self.label, col, self.a_val, R, self.separator, self.b_val, self.metric)
    }
}

struct Ffraction {
    label: String,
    a_val: f64,
    b_val: f64,
    separator: char,
    metric: String,
}

impl Ffraction {
    fn new(label: String, first_value: f64, second_value: f64, separator: char, met: String) -> Ffraction {
        Ffraction { label, a_val: first_value, b_val: second_value, separator, metric: met }
    }

    fn putfrac(&self) -> String {
        let used_percent = if self.b_val == 0.0 {
            0.0
        } else {
            (self.a_val / self.b_val) * 100.0
        };

        let col = if self.b_val == 0.0 {
            FDARED
        } else if used_percent < 25.0 {
            FGREEN
        } else if used_percent < 50.0 {
            FAMBER
        } else if used_percent < 90.0 {
            FRED
        } else {
            FDARED
        };

        format!("       {}: {}{:.1}{} {} {:.1} {}", self.label, col, self.a_val, R, self.separator, self.b_val, self.metric)
    }
}

struct ProcessRow {
    pid: u32,
    name: String,
    cpu: f32,
    ram_mb: u64,
    gpu: u32,
}

fn get_gpu_percent_by_pid(dev: &nvml_wrapper::Device) -> HashMap<u32, u32> {
    let mut map = HashMap::new();
    if let Ok(samples) = dev.process_utilization_stats(None) {
        for sample in samples {
            map.insert(sample.pid, sample.sm_util);
        }
    }
    map
}

fn process_table_row(row: &ProcessRow, is_selected: bool) -> String {
    let mut name = row.name.clone();
    if name.chars().count() > 14 {
        name = name.chars().take(14).collect();
    }

    let line = format!(
        "{:<14} {:>5.1}% {:>6}MB {:>4}%",
        name, row.cpu, row.ram_mb, row.gpu
    );

    if is_selected {
        format!("{}{}{}", HIGHLIGHT, line, R)
    } else {
        line
    }
}

fn ascii_art() {
    println!("                            \x1b[38;2;0;102;204m/$$$$$$$$$\u{001b}[0m/$$$$$$$$$                  /$$                ");
    println!("                            \x1b[38;2;0;102;204m|__  $$__/\u{001b}[0m|__  $$__/                 | $$                ");
    println!("                               \x1b[38;2;0;102;204m| $$\u{001b}[0m      | $$  /$$$$$$   /$$$$$$$| $$   /$$  /$$$$$$$");
    println!("                               \x1b[38;2;0;102;204m| $$\u{001b}[0m      | $$ |____  $$ /$$_____/| $$  /$$/ /$$_____/");
    println!("                               \x1b[38;2;0;102;204m| $$\u{001b}[0m      | $$  /$$$$$$$|  $$$$$$ | $$$$$$/ |  $$$$$$ ");
    println!("                               \x1b[38;2;0;102;204m| $$\u{001b}[0m      | $$ /$$__  $$ \\____  $$| $$_  $$  \\____  $$");
    println!("                               \x1b[38;2;0;102;204m| $$\u{001b}[0m      | $$|  $$$$$$$ /$$$$$$$/| $$ \\  $$ /$$$$$$$/");
    println!("                               \x1b[38;2;0;102;204m|__/\u{001b}[0m      |__/ \\_______/|_______/ |__/  \\__/|_______/ ");
}

fn draw_screen(
    cpu_usg: f32,
    ram_used: u64,
    ram_max: u64,
    gpu_usg: u32,
    vram_used: u64,
    vram_max: u64,
    disks: &Disks,
    processes: &Vec<ProcessRow>,
    selected: i32,
    noproc: bool,
) {
    ascii_art();
    println!("\n\n");

    let left_margin = if noproc {
        format!("{}{}", MARGIN, " ".repeat(20))
    } else {
        MARGIN.to_string()
    };

    let header = if noproc {
        format!(
            "{}╭{}╮",
            left_margin,
            label_dashes("Preformance", PURP, LEFT_WIDTH)
        )
    } else {
        format!(
            "{}╭{}┬{}╮",
            left_margin,
            label_dashes("Preformance", PURP, LEFT_WIDTH),
            label_dashes("Proccesses", CYAN, RIGHT_WIDTH)
        )
    };
    println!("{}", header);

    let mut cpu_bar = Bar::new(String::from("cpu"));
    cpu_bar.value = cpu_usg;

    let mut gpu_bar = Bar::new(String::from("gpu"));
    gpu_bar.value = gpu_usg as f32;

    let ram_frac = Fraction::new(String::from("ram"), ram_used / I_GIGABYTE, ram_max / I_GIGABYTE, '/', String::from("Gigabytes"));
    let vram_frac = Fraction::new(String::from("vram"), vram_used / I_GIGABYTE, vram_max / I_GIGABYTE, '/', String::from("Gigabytes"));

    let mut left_lines: Vec<String> = Vec::new();
    left_lines.push(format!("{}│{}│", left_margin, pad(cpu_bar.putbar(), LEFT_WIDTH)));
    left_lines.push(format!("{}│{}│", left_margin, pad(gpu_bar.putbar(), LEFT_WIDTH)));
    left_lines.push(format!("{}│{}│", left_margin, pad(String::new(), LEFT_WIDTH)));
    left_lines.push(format!("{}│{}│", left_margin, pad(ram_frac.putfrac(), LEFT_WIDTH)));
    left_lines.push(format!("{}│{}│", left_margin, pad(vram_frac.putfrac(), LEFT_WIDTH)));
    left_lines.push(format!("{}│{}│", left_margin, pad(String::new(), LEFT_WIDTH)));
    left_lines.push(format!("{}├{}│", left_margin, label_dashes("Disks", GREENT, LEFT_WIDTH)));

    for disk in disks {
        let partition = disk.mount_point().to_string_lossy().replace('\\', "").replace(':', "");
        let used_gb = (disk.total_space() - disk.available_space()) as f64 / GIGABYTE;
        let total_gb = disk.total_space() as f64 / GIGABYTE;
        let disk_frac = Ffraction::new(partition, used_gb, total_gb, '/', String::from("GB"));
        left_lines.push(format!("{}│{}│", left_margin, pad(disk_frac.putfrac(), LEFT_WIDTH)));
    }

    left_lines.push(format!("{}╰{}{}", left_margin, "─".repeat(LEFT_WIDTH), if noproc { "╯" } else { "┤" }));

    if noproc {
        for line in &left_lines {
            println!("{}", line);
        }
        return;
    }

    let mut right_lines: Vec<String> = Vec::new();
    let table_head = format!("{:<14} {:>6} {:>8} {:>5}", "name", "cpu", "ram", "gpu");
    right_lines.push(format!("{}│", pad(table_head, RIGHT_WIDTH)));

    let mut row_index: i32 = 0;
    while row_index < PROCESS_ROW_COUNT {
        let index_in_list = row_index as usize;
        if index_in_list < processes.len() {
            let process_line = process_table_row(&processes[index_in_list], row_index == selected);
            right_lines.push(format!("{}│", pad(process_line, RIGHT_WIDTH)));
        } else {
            right_lines.push(format!("{}│", pad(String::new(), RIGHT_WIDTH)));
        }
        row_index = row_index + 1;
    }

    let blank_left = format!("{}{}│", MARGIN, " ".repeat(LEFT_WIDTH + 1));
    let total_rows = if left_lines.len() > right_lines.len() { left_lines.len() } else { right_lines.len() };

    let mut row = 0;
    while row < total_rows {
        let left_part = if row < left_lines.len() { left_lines[row].clone() } else { blank_left.clone() };
        let right_part = if row < right_lines.len() { right_lines[row].clone() } else { String::new() };
        println!("{}{}", left_part, right_part);
        row = row + 1;
    }

    println!("{}{}╰{}╯", MARGIN, " ".repeat(LEFT_WIDTH + 1), "─".repeat(RIGHT_WIDTH));
}

fn key_just_pressed(vk: i32, state: &mut bool) -> bool {
    let down = unsafe { (GetAsyncKeyState(vk) as u16) & 0x8000 != 0 };
    let just_pressed = down && !*state;
    *state = down;
    just_pressed
}

fn main() {
    let mut sys = System::new();
    let nvml = Nvml::init().ok();
    let mut selected: i32 = 0;
    let mut process_list: Vec<ProcessRow> = Vec::new();

    let args: Vec<String> = std::env::args().collect();
    let noproc = args.iter().any(|a| a == "-noproc");

    let mut up_down = false;
    let mut down_down = false;
    let mut enter_down = false;
    let mut q_down = false;

    loop {
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let cpu_usg = sys.global_cpu_usage();
        let ram_used = sys.used_memory();
        let ram_max = sys.total_memory();

        let dev = nvml.as_ref().unwrap().device_by_index(0).unwrap();

        let gpu_usg = dev.utilization_rates().unwrap().gpu;
        let vram_used = dev.memory_info().unwrap().used;
        let vram_max = dev.memory_info().unwrap().total;

        let gpu_by_pid = get_gpu_percent_by_pid(&dev);

        let disks = Disks::new_with_refreshed_list();

        let mut current_apps: Vec<ProcessRow> = Vec::new();
        for (pid, process) in sys.processes() {
            let pid_number = pid.as_u32();
            let gpu_value = *gpu_by_pid.get(&pid_number).unwrap_or(&0);
            let name_str = process.name().to_string_lossy().to_string();
            let name_lower = name_str.to_lowercase();

            let is_system_background = name_lower.contains("svchost")
                || name_lower.contains("system")
                || name_lower.contains("service")
                || name_lower.contains("helper")
                || name_lower.contains("driver")
                || name_lower.contains("runtime")
                || name_lower.contains("daemon")
                || name_lower.contains("wmiprvse")
                || name_lower.contains("lsass")
                || name_lower.contains("csrss")
                || name_lower.contains("smss")
                || name_lower.contains("wininit")
                || name_lower.contains("services")
                || name_lower.contains("spoolsv")
                || name_lower.contains("conhost")
                || name_lower.contains("armoury")
                || name_lower.contains("aac")
                || name_lower.contains("explorer")
                || name_lower.contains("taskmgr")
                || name_lower.contains("searchhost")
                || name_lower.contains("nvidia")
                || name_lower.contains("openconsole")
                || name_lower.contains("snippingtool")
                || name_lower.contains("nvcontainer")
                || name_lower.contains("asus")
                || name_lower.contains("powershell")
                || name_lower.contains("securityhealth")
                || name_lower.contains("eosoverlay")
                || name_lower.contains("msedgewebview")
                || name_lower.contains("xboxpcappft")
                || name_lower.contains("tip")
                || name_lower.contains("background")
                || name_lower.contains("acpowernotific");
            let has_exe = process.exe().is_some();
            let session_id = process.session_id().map(|s| s.as_u32()).unwrap_or(0);
            let is_app = has_exe && !is_system_background && session_id > 0;

            if is_app {
                current_apps.push(ProcessRow {
                    pid: pid_number,
                    name: name_str,
                    cpu: process.cpu_usage(),
                    ram_mb: process.memory() / 1024 / 1024,
                    gpu: gpu_value,
                });
            }
        }

        let mut current_pids: Vec<u32> = current_apps.iter().map(|p| p.pid).collect();
        let mut existing_pids: Vec<u32> = process_list.iter().map(|p| p.pid).collect();

        current_pids.sort();
        existing_pids.sort();

        if current_pids != existing_pids {
            process_list = current_apps;
            process_list.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap());
        } else {
            for row in process_list.iter_mut() {
                if let Some(app) = current_apps.iter().find(|p| p.pid == row.pid) {
                    row.cpu = app.cpu;
                    row.ram_mb = app.ram_mb;
                    row.gpu = app.gpu;
                }
            }
        }

        if selected < 0 {
            selected = 0;
        }
        if process_list.len() > 0 && selected >= process_list.len() as i32 {
            selected = process_list.len() as i32 - 1;
        }

        print!("\x1B[2J\x1B[1;1H");
        draw_screen(cpu_usg, ram_used, ram_max, gpu_usg, vram_used, vram_max, &disks, &process_list, selected, noproc);

        let frame_start = Instant::now();
        let mut should_quit = false;

        while frame_start.elapsed() < Duration::from_millis(WAIT_TIME) {
            if key_just_pressed(VK_UP.0 as i32, &mut up_down) {
                if selected > 0 {
                    selected = selected - 1;
                }
            }
            if key_just_pressed(VK_DOWN.0 as i32, &mut down_down) {
                if selected < process_list.len() as i32 - 1 {
                    selected = selected + 1;
                }
            }
            if key_just_pressed(VK_RETURN.0 as i32, &mut enter_down) {
                let index_in_list = selected as usize;
                if index_in_list < process_list.len() {
                    let target_pid = process_list[index_in_list].pid;
                    if let Some(target_process) = sys.process(Pid::from_u32(target_pid)) {
                        target_process.kill();
                    }
                }
            }
            if key_just_pressed(VK_Q, &mut q_down) {
                should_quit = true;
            }

            thread::sleep(Duration::from_millis(POLL_STEP));
        }

        if should_quit {
            break;
        }

        thread::sleep(Duration::from_millis(0));
    }
}