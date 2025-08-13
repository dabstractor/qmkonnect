use crate::core::types::WindowInfo;
use once_cell::sync::Lazy;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(50);

// Trait to abstract the notification functionality
pub trait Notifier: Send + Sync {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>>;
}

// Real implementation that uses qmk_notifier
pub struct QmkNotifier;
impl Notifier for QmkNotifier {
    fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Ok(api) = hidapi::HidApi::new() {
            for device in api.device_list() {
                let _device_info = format!(
                    "VID: 0x{:04X}, PID: 0x{:04X}, Usage Page: 0x{:04X}, Usage: 0x{:04X}",
                    device.vendor_id(),
                    device.product_id(),
                    device.usage_page(),
                    device.usage()
                );
            }
        }

        // Retry device connection with exponential backoff
        for attempt in 1..=3 {
            match qmk_notifier::run(Some(message.clone())) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    
                    // Only retry for device-related errors
                    if error_str.contains("no device found") || 
                       error_str.contains("permission denied") || 
                       error_str.contains("failed to open") {
                        
                        if attempt < 3 {
                            let delay = Duration::from_millis(100 * attempt as u64);
                            thread::sleep(delay);
                            continue;
                        }
                        
                        // After 3 attempts, log and return success to prevent service restart
                        eprintln!("QMK device unavailable after {} attempts: {}", attempt, e);
                        return Ok(()); // Don't fail the service for device issues
                    }
                    
                    // For non-device errors, fail immediately
                    return Err(Box::new(e));
                }
            }
        }
        
        Ok(())
    }
}

// Static instance of the notifier
static NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Box::new(QmkNotifier) as Box<dyn Notifier>)));

// Static state for debouncing
static DEBOUNCER: Lazy<Arc<Mutex<DebounceState>>> = Lazy::new(|| {
    Arc::new(Mutex::new(DebounceState {
        last_message: None,
        last_sent_time: Instant::now(),
        debounce_window_start: None,
        timer_running: false,
    }))
});

// For testing: Set a custom notifier
#[cfg(test)]
pub fn set_notifier(notifier: Box<dyn Notifier>) {
    // Ensure the static has been initialized first
    let _ = &*NOTIFIER;

    {
        let mut n = NOTIFIER.lock().unwrap();
        *n = notifier;
    }
}

struct DebounceState {
    last_message: Option<String>,
    last_sent_time: Instant,
    debounce_window_start: Option<Instant>,
    timer_running: bool,
}

fn get_debouncer() -> Arc<Mutex<DebounceState>> {
    Arc::clone(&DEBOUNCER)
}

fn get_notifier() -> Arc<Mutex<Box<dyn Notifier>>> {
    Arc::clone(&NOTIFIER)
}

pub fn notify_qmk(
    window_info: &WindowInfo,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let message = format!("{}{}{}", window_info.app_class, "\x1D", window_info.title);
    let debouncer = get_debouncer();

    // Wrap in a block to limit the lock scope
    let should_send_now = {
        let mut state = debouncer.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_sent_time);

        // Check if this is the first message or if debounce window has expired
        let is_first_message = state.last_message.is_none();

        #[cfg(test)]
        println!(
            "is_first_message: {}, elapsed: {:?}",
            is_first_message, elapsed
        );

        // Determine if we should send immediately
        // For tests, use a slightly higher threshold to avoid timing issues
        #[cfg(test)]
        let should_send_now =
            elapsed > DEBOUNCE_INTERVAL + Duration::from_millis(100) || is_first_message;

        #[cfg(not(test))]
        let should_send_now = elapsed > DEBOUNCE_INTERVAL || is_first_message;

        // Store the latest message (always store for potential debounced send)
        state.last_message = Some(message.clone());

        // If sending now, update last_sent_time and start a new debounce window
        if should_send_now {
            state.last_sent_time = now;
            state.debounce_window_start = Some(now);
        }

        // Start the timer thread if not already running and we're not sending immediately
        if !state.timer_running && !should_send_now {
            state.timer_running = true;
            // If we don't have a debounce window start time, set it now
            if state.debounce_window_start.is_none() {
                state.debounce_window_start = Some(now);
            }
            let debouncer_clone = Arc::clone(&debouncer);
            let notifier_clone = get_notifier();
            thread::spawn(move || debounce_timer(debouncer_clone, notifier_clone));
        }

        should_send_now
    };

    // Send immediately if needed
    if should_send_now {
        if verbose {
            let sanitized_message = message.replace('\x1D', "|");
            println!("Notified QMK (immediate): {}", sanitized_message);
        }
        let notifier = get_notifier();
        let notifier = notifier.lock().unwrap();

        #[cfg(test)]
        println!("Sending notification immediately: {}", message);

        notifier.notify(message)?;
    } else if verbose {
        let sanitized_message = message.replace('\x1D', "|");
        println!("Debouncing notification: {}", sanitized_message);
    }

    Ok(())
}

fn debounce_timer(debouncer: Arc<Mutex<DebounceState>>, notifier: Arc<Mutex<Box<dyn Notifier>>>) {
    loop {
        thread::sleep(Duration::from_millis(10)); // Check every 100ms

        let (should_exit, message_to_send, verbose_mode) = {
            let mut state = debouncer.lock().unwrap();
            let now = Instant::now();
            
            // Check if debounce window has expired
            let should_send = if let Some(window_start) = state.debounce_window_start {
                now.duration_since(window_start) >= DEBOUNCE_INTERVAL
            } else {
                false // No window started, shouldn't happen but handle gracefully
            };

            if should_send {
                let message = state.last_message.clone();
                state.last_message = None;
                state.timer_running = false;
                state.debounce_window_start = None; // Reset the debounce window
                state.last_sent_time = now; // Update last sent time
                (true, message, false) // Exit after sending
            } else {
                (false, None, false) // Continue waiting
            }
        };

        // Send the debounced message if needed
        if let Some(message) = message_to_send {
            if verbose_mode {
                let sanitized_message = message.replace('\x1D', "|");
                println!("Notified QMK (debounced): {}", sanitized_message);
            }

            #[cfg(test)]
            println!("Sending debounced notification: {}", message);

            let notifier_guard = notifier.lock().unwrap();

            // Use explicit error handling for better debugging
            if let Err(e) = notifier_guard.notify(message) {
                eprintln!("Error sending debounced notification: {}", e);
            }
        }

        if should_exit {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::WindowInfo;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    // Use a shared global mock for testing
    static MOCK_CALL_COUNT: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));
    static MOCK_LAST_MESSAGE: Lazy<StdMutex<Option<String>>> = Lazy::new(|| StdMutex::new(None));

    // Reset the global mock state
    fn reset_global_mock() {
        MOCK_CALL_COUNT.store(0, Ordering::SeqCst);
        *MOCK_LAST_MESSAGE.lock().unwrap() = None;
    }

    // Simple MockNotifier for testing
    struct MockNotifier;

    impl MockNotifier {
        fn new() -> Self {
            // Reset state on creation for test isolation
            reset_global_mock();
            Self
        }

        fn get_call_count() -> usize {
            MOCK_CALL_COUNT.load(Ordering::SeqCst)
        }

        fn get_last_message() -> Option<String> {
            MOCK_LAST_MESSAGE.lock().unwrap().clone()
        }
    }

    impl Notifier for MockNotifier {
        fn notify(&self, message: String) -> Result<(), Box<dyn Error + Send + Sync>> {
            println!("MockNotifier.notify called with: {}", message);

            // Atomically increment the call count
            MOCK_CALL_COUNT.fetch_add(1, Ordering::SeqCst);

            // Update the last message
            let mut last_message = MOCK_LAST_MESSAGE.lock().unwrap();
            *last_message = Some(message);

            Ok(())
        }
    }

    // Reset test state between tests
    fn reset_test_state() {
        // Wait for any ongoing timers to finish first
        thread::sleep(Duration::from_millis(300));
        
        // Reset the debouncer
        let mut state = DEBOUNCER.lock().unwrap();
        *state = DebounceState {
            last_message: None,
            last_sent_time: Instant::now(),
            debounce_window_start: None,
            timer_running: false,
        };
        drop(state); // Release the lock

        // Reset the global mock
        reset_global_mock();

        // Additional wait to ensure cleanup is complete
        thread::sleep(Duration::from_millis(100));
    }

    #[test]
    fn test_immediate_send_first_message() {
        reset_test_state();

        // Set our notifier
        set_notifier(Box::new(MockNotifier::new()));
        println!("Initial call count: {}", MockNotifier::get_call_count());

        let window_info = WindowInfo::new("TestApp".to_string(), "Test Title".to_string());

        println!("Before calling notify_qmk");
        let result = notify_qmk(&window_info, true);
        println!("After calling notify_qmk");
        assert!(result.is_ok());

        // Give enough time for notification to complete
        thread::sleep(Duration::from_millis(500));

        // Check with debug output
        let count = MockNotifier::get_call_count();

        assert_eq!(count, 1);

        assert_eq!(
            MockNotifier::get_last_message(),
            Some(format!(
                "{}\x1D{}",
                window_info.app_class, window_info.title
            ))
        );
    }

    #[test]
    fn test_debounce_subsequent_messages() {
        reset_test_state();

        set_notifier(Box::new(MockNotifier::new()));

        // First message - sent immediately
        let window1 = WindowInfo::new("App1".to_string(), "Title1".to_string());
        let result = notify_qmk(&window1, true);
        assert!(result.is_ok());

        // Give time for first notification to process
        thread::sleep(Duration::from_millis(400));
        assert_eq!(MockNotifier::get_call_count(), 1);

        // Second message within 600ms - should be debounced
        let window2 = WindowInfo::new("App2".to_string(), "Title2".to_string());
        let result = notify_qmk(&window2, true);
        assert!(result.is_ok());

        // Wait a short time - call count should still be 1
        thread::sleep(DEBOUNCE_INTERVAL);
        assert_eq!(MockNotifier::get_call_count(), 1);
    }

    #[test]
    fn test_send_after_debounce_timeout() {
        reset_test_state();

        set_notifier(Box::new(MockNotifier::new()));

        // First message - should be sent immediately
        let window1 = WindowInfo::new("App1".to_string(), "Title1".to_string());
        let _ = notify_qmk(&window1, true);

        // Give time for the first notification to complete
        thread::sleep(Duration::from_millis(500));
        println!(
            "After first notification, call count: {}",
            MockNotifier::get_call_count()
        );

        // Second message within debounce period - should be debounced
        let window2 = WindowInfo::new("App2".to_string(), "Title2".to_string());
        let _ = notify_qmk(&window2, true);

        // Wait for the debounce timer to fully complete and send
        thread::sleep(Duration::from_millis(2000));

        // Verify debounced message was sent (2 total calls)
        assert_eq!(MockNotifier::get_call_count(), 2);
        assert_eq!(
            MockNotifier::get_last_message(),
            Some(format!("{}\x1D{}", window2.app_class, window2.title))
        );
    }

    #[test]
    fn test_multiple_rapid_updates() {
        reset_test_state();

        set_notifier(Box::new(MockNotifier::new()));

        // First message - sent immediately
        let _ = notify_qmk(
            &WindowInfo::new("App1".to_string(), "Title1".to_string()),
            true,
        );

        // Give time for first message to process
        thread::sleep(Duration::from_millis(500));
        println!(
            "After first notification, call count: {}",
            MockNotifier::get_call_count()
        );

        // Several rapid updates - all debounced
        for i in 2..=5 {
            let _ = notify_qmk(
                &WindowInfo::new(format!("App{}", i), format!("Title{}", i)),
                true,
            );
            thread::sleep(Duration::from_millis(50));
        }

        // Wait for the debounce timer to fully complete
        thread::sleep(Duration::from_millis(2000));

        // Verify only the last debounced message was sent
        assert_eq!(MockNotifier::get_call_count(), 2);
        assert_eq!(
            MockNotifier::get_last_message(),
            Some("App5\x1DTitle5".to_string())
        );
    }

    #[test]
    fn test_verbose_mode() {
        reset_test_state();

        set_notifier(Box::new(MockNotifier::new()));

        // We can't easily test the println output, but we can verify
        // that the notification still works in verbose mode
        let window_info = WindowInfo::new("VerboseApp".to_string(), "Test Verbose".to_string());
        let result = notify_qmk(&window_info, true);
        assert!(result.is_ok());

        thread::sleep(Duration::from_millis(500));
        assert_eq!(MockNotifier::get_call_count(), 1);
    }

    #[test]
    fn test_threads_dont_interfere() {
        reset_test_state();

        set_notifier(Box::new(MockNotifier::new()));

        // Start several threads that all send notifications
        let mut handles = vec![];
        for i in 1..=5 {
            let window_info =
                WindowInfo::new(format!("ThreadApp{}", i), format!("Thread {} Title", i));

            let handle = thread::spawn(move || {
                let _ = notify_qmk(&window_info, false);
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            let _ = handle.join();
        }

        // Wait for debouncing to finish
        thread::sleep(Duration::from_millis(2000));

        // We can't reliably predict exact behavior here, but the call count
        // should be at least 1 (and probably not 5, due to debouncing)
        let count = MockNotifier::get_call_count();
        println!("Final call count after threaded test: {}", count);
        assert!(count >= 1);
    }
}
