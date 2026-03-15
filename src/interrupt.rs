use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

// Flags pour les différentes méthodes d'interruption
static CTRL_C_REQUESTED: AtomicBool = AtomicBool::new(false);
static ESCAPE_REQUESTED: AtomicBool = AtomicBool::new(false);
static CTRLC_SET: Once = Once::new();

/// Initialise les gestionnaires d'interruption (Ctrl+C)
pub fn init_interrupt_handler() {
    // Configurer Ctrl+C
    CTRLC_SET.call_once(|| {
        ctrlc::set_handler(move || {
            CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
        })
        .expect("Failed to set Ctrl+C handler");
    });
}

/// Vérifie si l'utilisateur a demandé une interruption (Ctrl+C ou Échap)
pub fn is_interrupt_requested() -> bool {
    CTRL_C_REQUESTED.load(Ordering::SeqCst) || ESCAPE_REQUESTED.load(Ordering::SeqCst)
}

/// Réinitialise tous les flags d'interruption
pub fn reset_interrupt() {
    CTRL_C_REQUESTED.store(false, Ordering::SeqCst);
    ESCAPE_REQUESTED.store(false, Ordering::SeqCst);
}

/// Vérifie l'interruption et retourne vrai si elle a été demandée.
/// Si une interruption est détectée, le flag est réinitialisé automatiquement.
pub fn check_interrupt() -> bool {
    // Vérifier si la touche Échap est pressée maintenant
    if check_escape_pressed() {
        ESCAPE_REQUESTED.store(true, Ordering::SeqCst);
    }
    
    // Vérifier Ctrl+C et Échap (depuis les flags)
    let ctrl_c = CTRL_C_REQUESTED.swap(false, Ordering::SeqCst);
    let escape = ESCAPE_REQUESTED.swap(false, Ordering::SeqCst);
    
    ctrl_c || escape
}

/// Vérifie spécifiquement si la touche Échap a été pressée (à appeler périodiquement)
pub fn check_escape_pressed() -> bool {
    use crossterm::event::{self, Event, KeyCode, KeyEvent};
    use std::time::Duration;
    
    // Vérifier sans bloquer si la touche Échap est pressée
    if event::poll(Duration::from_millis(0)).unwrap_or(false) {
        if let Ok(Event::Key(KeyEvent { code: KeyCode::Esc, .. })) = event::read() {
            return true;
        }
    }
    false
}

/// Exécute une action avec vérification périodique d'interruption.
/// La fonction `check` est appelée périodiquement ; si elle retourne true, on arrête.
pub fn run_with_interrupt_check<F, C>(action: F, mut check: C) -> bool
where
    F: FnOnce() -> bool,
    C: FnMut() -> bool,
{
    reset_interrupt();
    let _result = action();
    // Si l'action est terminée, vérifier si une interruption a été demandée pendant l'exécution
    if is_interrupt_requested() {
        reset_interrupt();
        return true;
    }
    // Sinon, appeler le check personnalisé
    if check() {
        return true;
    }
    false
}