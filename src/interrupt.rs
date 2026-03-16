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
#[allow(dead_code)]
pub fn run_with_interrupt_check<F, C>(action: F, mut check: C) -> bool
where
    F: FnOnce() -> bool,
    C: FnMut() -> bool,
{
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_interrupt_flags() {
        // Réinitialiser les flags avant le test
        reset_interrupt();
        assert!(!is_interrupt_requested());
        
        // Simuler Ctrl+C
        CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
        assert!(is_interrupt_requested());
        
        // Réinitialiser
        reset_interrupt();
        assert!(!is_interrupt_requested());
        
        // Simuler Échap
        ESCAPE_REQUESTED.store(true, Ordering::SeqCst);
        assert!(is_interrupt_requested());
        
        // Réinitialiser
        reset_interrupt();
        assert!(!is_interrupt_requested());
        
        // Les deux flags
        CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
        ESCAPE_REQUESTED.store(true, Ordering::SeqCst);
        assert!(is_interrupt_requested());
        
        reset_interrupt();
    }

    #[test]
    fn test_check_interrupt() {
        reset_interrupt();
        // Pas d'interruption
        assert!(!check_interrupt());
        
        // Simuler Ctrl+C
        CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
        assert!(check_interrupt());
        // Après check_interrupt, le flag est réinitialisé
        assert!(!CTRL_C_REQUESTED.load(Ordering::SeqCst));
        
        // Simuler Échap
        ESCAPE_REQUESTED.store(true, Ordering::SeqCst);
        assert!(check_interrupt());
        assert!(!ESCAPE_REQUESTED.load(Ordering::SeqCst));
        
        // Vérifier que check_interrupt réinitialise les deux flags
        CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
        ESCAPE_REQUESTED.store(true, Ordering::SeqCst);
        assert!(check_interrupt());
        assert!(!CTRL_C_REQUESTED.load(Ordering::SeqCst));
        assert!(!ESCAPE_REQUESTED.load(Ordering::SeqCst));
        
        reset_interrupt();
    }

    #[test]
    fn test_run_with_interrupt_check_no_interrupt() {
        reset_interrupt();
        let action_executed = std::cell::Cell::new(false);
        let check_called = std::cell::Cell::new(false);
        
        let interrupted = run_with_interrupt_check(
            || {
                action_executed.set(true);
                false // action ne demande pas d'arrêt
            },
            || {
                check_called.set(true);
                false // check ne demande pas d'arrêt
            },
        );
        
        assert!(action_executed.get());
        assert!(check_called.get());
        assert!(!interrupted);
        reset_interrupt();
    }

    #[test]
    fn test_run_with_interrupt_check_with_interrupt_during_action() {
        reset_interrupt();
        // Simuler une interruption pendant l'action
        CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
        
        let interrupted = run_with_interrupt_check(
            || false,
            || false,
        );
        
        assert!(interrupted);
        // L'interruption a été réinitialisée par run_with_interrupt_check
        assert!(!is_interrupt_requested());
        reset_interrupt();
    }

    #[test]
    fn test_run_with_interrupt_check_with_check_returning_true() {
        reset_interrupt();
        let interrupted = run_with_interrupt_check(
            || false,
            || true, // check demande l'arrêt
        );
        
        assert!(interrupted);
        reset_interrupt();
    }

    // Note: les tests pour init_interrupt_handler, check_escape_pressed
    // ne sont pas inclus car ils dépendent de ctrlc et crossterm.
}