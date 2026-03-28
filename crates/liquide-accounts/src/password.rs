//! Password policy enforcement and strength estimation.

use std::fmt;

/// Estimated strength of a password.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PasswordStrength {
    /// Trivially guessable.
    Weak,
    /// Below recommended thresholds but not trivial.
    Fair,
    /// Meets basic requirements.
    Good,
    /// Exceeds basic requirements with good diversity.
    Strong,
    /// Excellent length and character diversity.
    VeryStrong,
}

impl fmt::Display for PasswordStrength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Weak => write!(f, "Weak"),
            Self::Fair => write!(f, "Fair"),
            Self::Good => write!(f, "Good"),
            Self::Strong => write!(f, "Strong"),
            Self::VeryStrong => write!(f, "Very Strong"),
        }
    }
}

/// Configurable password policy with strength checking.
#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    /// Minimum number of characters.
    pub min_length: usize,
    /// Require at least one uppercase letter.
    pub require_uppercase: bool,
    /// Require at least one lowercase letter.
    pub require_lowercase: bool,
    /// Require at least one decimal digit.
    pub require_digit: bool,
    /// Require at least one special (non-alphanumeric) character.
    pub require_special: bool,
    /// Force password change after this many days (if set).
    pub max_age_days: Option<u32>,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: false,
            max_age_days: None,
        }
    }
}

impl PasswordPolicy {
    /// Check a password against this policy. Returns `Ok(())` if the
    /// password satisfies all rules, or `Err` with a list of violations.
    pub fn check(&self, password: &str) -> Result<(), Vec<String>> {
        let mut violations = Vec::new();

        if password.len() < self.min_length {
            violations.push(format!(
                "must be at least {} characters (got {})",
                self.min_length,
                password.len()
            ));
        }

        if self.require_uppercase && !password.chars().any(|c| c.is_ascii_uppercase()) {
            violations.push("must contain at least one uppercase letter".to_string());
        }

        if self.require_lowercase && !password.chars().any(|c| c.is_ascii_lowercase()) {
            violations.push("must contain at least one lowercase letter".to_string());
        }

        if self.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
            violations.push("must contain at least one digit".to_string());
        }

        if self.require_special
            && !password
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && !c.is_ascii_whitespace())
        {
            violations.push("must contain at least one special character".to_string());
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    /// Estimate the strength of a password based on length, character
    /// diversity, and entropy.
    pub fn strength(&self, password: &str) -> PasswordStrength {
        if password.is_empty() {
            return PasswordStrength::Weak;
        }

        let entropy = estimate_entropy(password);

        // Score thresholds based on entropy bits:
        //   < 28  => Weak
        //   < 36  => Fair
        //   < 60  => Good
        //   < 80  => Strong
        //   >= 80 => VeryStrong
        if entropy < 28.0 {
            PasswordStrength::Weak
        } else if entropy < 36.0 {
            PasswordStrength::Fair
        } else if entropy < 60.0 {
            PasswordStrength::Good
        } else if entropy < 80.0 {
            PasswordStrength::Strong
        } else {
            PasswordStrength::VeryStrong
        }
    }
}

/// Estimate Shannon entropy of a password in bits, adjusted for
/// character-class diversity.
fn estimate_entropy(password: &str) -> f64 {
    if password.is_empty() {
        return 0.0;
    }

    // Determine the effective alphabet size from character classes present.
    let mut pool_size: u32 = 0;
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password
        .chars()
        .any(|c| c.is_ascii() && !c.is_ascii_alphanumeric() && !c.is_ascii_whitespace());
    let has_unicode = password.chars().any(|c| !c.is_ascii());
    let has_space = password.chars().any(|c| c.is_ascii_whitespace());

    if has_lower {
        pool_size += 26;
    }
    if has_upper {
        pool_size += 26;
    }
    if has_digit {
        pool_size += 10;
    }
    if has_special {
        pool_size += 32; // common ASCII specials
    }
    if has_unicode {
        pool_size += 100; // conservative estimate for non-ASCII
    }
    if has_space {
        pool_size += 1;
    }
    if pool_size == 0 {
        pool_size = 1;
    }

    // Entropy = length * log2(pool_size), with a penalty for repeated chars.
    let len = password.len() as f64;
    let base_entropy = len * (pool_size as f64).log2();

    // Penalize if many characters are repeated.
    let unique_chars = {
        let mut chars: Vec<char> = password.chars().collect();
        chars.sort();
        chars.dedup();
        chars.len()
    };
    let uniqueness_ratio = unique_chars as f64 / password.len() as f64;

    // Scale entropy by uniqueness (heavily repeated passwords lose up to 40%).
    base_entropy * (0.6 + 0.4 * uniqueness_ratio)
}
