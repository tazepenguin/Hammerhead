use alloc::string::String;
use crate::{vga_buffer, keyboard, cpuinfo, meminfo, cmos, reboot, acpi, pci, sysctl};

const BANNER: &str = include_str!("banner.txt");
const PROMPT: &str = "Hammerhead>_ ";

// ── Shell entry point ─────────────────────────────────────────────────────────

pub fn run() -> ! {
    // We arrive here via switch_context() which was called from inside a
    // without_interrupts() closure.  That closure never gets to restore IF,
    // so we run with interrupts permanently disabled without this line.
    x86_64::instructions::interrupts::enable();

    vga_buffer::print_something(BANNER);
    vga_buffer::print_something("\n");

    loop {
        vga_buffer::print_something(PROMPT);
        let line = read_line();
        execute_script(&line);
    }
}

// ── Line reader ───────────────────────────────────────────────────────────────

/// Block until Enter is pressed, echoing characters and handling backspace.
/// Returns the typed string without the newline.
/// Uses a heap String so the line length is not statically bounded.
fn read_line() -> String {
    let mut buf = String::new();
    loop {
        if let Some(c) = keyboard::read_key() {
            match c {
                '\n' => {
                    vga_buffer::print_something("\n");
                    return buf;
                }
                '\x08' => {
                    if buf.pop().is_some() {
                        vga_buffer::print_something("\x08 \x08");
                    }
                }
                c if c.is_ascii() && buf.len() < 512 => {
                    buf.push(c);
                    vga_buffer::print_char(c);
                }
                _ => {}
            }
        } else {
            x86_64::instructions::hlt();
        }
    }
}

// ── Script parser ─────────────────────────────────────────────────────────────
//
// Grammar:
//   script  ::= segment { ';' segment }
//   segment ::= [ command ] [ '#' comment ]
//   command ::= word { word }
//
// ';' separates commands on one line.
// '#' starts a comment that extends to the next ';' or end-of-input.
//
// Examples:
//   version ; heap ; ticks
//   repeat 5 echo ping    # stress the allocator
//   cpuinfo ; regs ; date

fn execute_script(script: &str) {
    for segment in script.split(';') {
        let code = match segment.find('#') {
            Some(i) => &segment[..i],
            None    => segment,
        };
        let cmd = code.trim();
        if !cmd.is_empty() {
            execute_command(cmd);
        }
    }
}

// ── Command dispatch ──────────────────────────────────────────────────────────

/// Split `s` at the first whitespace run into `(verb, rest)`.
fn split_verb(s: &str) -> (&str, &str) {
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i + 1..].trim_start()),
        None    => (s, ""),
    }
}

fn execute_command(input: &str) {
    let (verb, rest) = split_verb(input);
    match verb {
        "" => {}

        // ── Meta ──────────────────────────────────────────────────────────────
        "help"  => cmd_help(),
        "clear" => vga_buffer::clear_screen(),
        "echo"  => { vga_buffer::print_something(rest); vga_buffer::print_something("\n"); }

        // ── System info ───────────────────────────────────────────────────────
        "version" => sysctl::version(),
        "ticks"   => cmd_ticks(),
        "uptime"  => cmd_uptime(),
        "regs"    => sysctl::regs(),
        "heap"    => cmd_heap(),
        "cpuinfo" => cpuinfo::run(),
        "meminfo" => meminfo::run(),
        "date"    => cmos::run(),

        // ── Hardware / diagnostics ────────────────────────────────────────────
        "pci"  => pci::run(),
        "peek" => cmd_peek(rest),
        "int3" => {
            vga_buffer::print_something("[int3] firing software breakpoint...\n");
            x86_64::instructions::interrupts::int3();
            vga_buffer::print_something("[int3] handler returned OK.\n");
        }

        // ── Scripting ─────────────────────────────────────────────────────────
        "repeat" => cmd_repeat(rest),

        // ── Power ─────────────────────────────────────────────────────────────
        "shutdown" => acpi::shutdown(),
        "reboot"   => reboot::reboot(),
        "halt"     => reboot::halt(),

        other => {
            vga_buffer::print_something(other);
            vga_buffer::print_something(": command not found\n");
        }
    }
}

// ── Individual command handlers ───────────────────────────────────────────────

fn cmd_help() {
    vga_buffer::print_something(concat!(
        "General\n",
        "  help              this text\n",
        "  clear             clear screen\n",
        "  echo <text>       print text\n",
        "  version           kernel build info\n",
        "\n",
        "Diagnostics\n",
        "  ticks             raw PIT tick counter\n",
        "  uptime            estimated uptime in seconds\n",
        "  regs              CR0/CR2/CR3/CR4/RFLAGS\n",
        "  heap              allocator statistics\n",
        "  cpuinfo           CPU vendor and brand string\n",
        "  meminfo           physical memory map\n",
        "  pci               PCI device list\n",
        "  peek <addr> [n]   hex-dump n bytes at virtual address\n",
        "  int3              test interrupt delivery via breakpoint\n",
        "  date              RTC date and time\n",
        "\n",
        "Scripting\n",
        "  repeat <N> <cmd>  run cmd N times (max 10000)\n",
        "  ;                 separate commands on one line\n",
        "  #                 comment to end of segment\n",
        "\n",
        "Power\n",
        "  shutdown / reboot / halt\n",
    ));
}

fn cmd_ticks() {
    let t = crate::interrupts::ticks();
    vga_buffer::print_fmt(format_args!(
        "Ticks: {}  (PIT @ {} Hz)\n",
        t, crate::interrupts::PIT_HZ,
    ));
}

fn cmd_uptime() {
    let t  = crate::interrupts::ticks();
    let hz = crate::interrupts::PIT_HZ;
    let s  = t / hz;
    let ms = (t % hz) * (1000 / hz);
    vga_buffer::print_fmt(format_args!(
        "Uptime: {}.{:03}s  ({} ticks @ {} Hz)\n",
        s, ms, t, hz,
    ));
}

fn cmd_heap() {
    let (used, free, total) = crate::allocator::heap_stats();
    let pct = if total > 0 { used * 100 / total } else { 0 };
    vga_buffer::print_fmt(format_args!(
        "Heap  total {:6} KiB  ({} B)\n\
              used  {:6} KiB  ({} B, {}%)\n\
              free  {:6} KiB  ({} B)\n",
        total / 1024, total,
        used  / 1024, used, pct,
        free  / 1024, free,
    ));
}

/// Hex-dump `count` bytes starting at virtual address `addr`.
///
/// Format — 8 bytes per row (fits in 80-column VGA):
///   ffffffff8000abcd: 48 61 6d 6d 65 72 68 65  |Hammerhe|
///
/// **Safety**: reading an unmapped address causes a page fault → double fault
/// → kernel panic.  Safe targets: 0xb8000 (VGA), 0x44444444_0000 (heap start).
fn cmd_peek(rest: &str) {
    let (addr_s, rest2) = split_verb(rest);
    if addr_s.is_empty() {
        vga_buffer::print_something(
            "Usage:   peek <hex_addr> [count]\n\
             Example: peek b8000 64\n\
             WARNING: invalid virtual addresses will triple-fault.\n",
        );
        return;
    }
    let hex = addr_s.trim_start_matches("0x").trim_start_matches("0X");
    let addr: usize = match usize::from_str_radix(hex, 16) {
        Ok(a)  => a,
        Err(_) => { vga_buffer::print_something("peek: invalid hex address\n"); return; }
    };
    let (count_s, _) = split_verb(rest2);
    let count: usize = count_s.parse::<usize>().unwrap_or(64).min(512).max(1);

    let mut offset = 0usize;
    while offset < count {
        vga_buffer::print_fmt(format_args!("{:016x}: ", addr + offset));

        let end = (offset + 8).min(count);

        // Hex column
        for i in offset..end {
            let b = unsafe { *((addr + i) as *const u8) };
            vga_buffer::print_fmt(format_args!("{:02x} ", b));
        }
        for _ in (end - offset)..8 {                 // pad short last row
            vga_buffer::print_something("   ");
        }

        // ASCII column
        vga_buffer::print_something(" |");
        for i in offset..end {
            let b = unsafe { *((addr + i) as *const u8) };
            vga_buffer::print_char(if (0x20..0x7f).contains(&b) { b as char } else { '.' });
        }
        vga_buffer::print_something("|\n");

        offset += 8;
    }
}

/// Run `subcmd` exactly N times (capped at 10 000).
///
///   repeat 3 echo hello
///   repeat 100 ticks ; heap    # ticks 100x, then heap once
fn cmd_repeat(rest: &str) {
    let (n_s, subcmd) = split_verb(rest);
    let n: usize = match n_s.parse::<usize>() {
        Ok(v)  => v.min(10_000),
        Err(_) => { vga_buffer::print_something("Usage: repeat <N> <command>\n"); return; }
    };
    if subcmd.is_empty() {
        vga_buffer::print_something("Usage: repeat <N> <command>\n");
        return;
    }
    for _ in 0..n {
        execute_command(subcmd);
    }
}