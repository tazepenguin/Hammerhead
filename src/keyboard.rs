use spin::Mutex;
use alloc::collections::VecDeque;
use spin::Lazy;

// A thread-safe queue to hold characters typed by the user
static CHAR_BUFFER: Lazy<Mutex<VecDeque<char>>> = Lazy::new(|| {
    Mutex::new(VecDeque::new())
});

/// Called by the interrupt handler to add characters to the queue
pub fn add_char(c: char) {
    let mut buffer = CHAR_BUFFER.lock();
    if buffer.len() < 256 { // Bound buffer size to protect memory
        buffer.push_back(c);
    }
}

/// Called by the shell to pop characters from the queue safely
pub fn read_key() -> Option<char> {
    // Disable interrupts briefly while pulling a key to prevent deadlocks
    x86_64::instructions::interrupts::without_interrupts(|| {
        CHAR_BUFFER.lock().pop_front()
    })
}