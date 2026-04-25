//! App reviews and ratings.

/// A user review of an application.
#[derive(Debug, Clone)]
pub struct Review {
    /// Reviewer name.
    pub author: String,
    /// Rating (1..=5 stars).
    pub rating: u8,
    /// Review text.
    pub text: String,
    /// Timestamp (epoch seconds).
    pub timestamp: u64,
    /// Whether this review was helpful (upvotes).
    pub helpful_count: u32,
}

impl Review {
    /// Create a new review.
    pub fn new(
        author: impl Into<String>,
        rating: u8,
        text: impl Into<String>,
        timestamp: u64,
    ) -> crate::Result<Self> {
        if rating == 0 || rating > 5 {
            return Err(crate::SoftwareCenterError::InvalidRating(rating));
        }
        Ok(Self {
            author: author.into(),
            rating,
            text: text.into(),
            timestamp,
            helpful_count: 0,
        })
    }
}

/// Review statistics for a package.
#[derive(Debug, Clone)]
pub struct ReviewStats {
    /// Total number of reviews.
    pub total_reviews: usize,
    /// Average rating (1.0..=5.0).
    pub average_rating: f64,
    /// Distribution of ratings (index 0 = 1 star, index 4 = 5 stars).
    pub distribution: [usize; 5],
}

impl ReviewStats {
    /// Compute stats from a list of reviews.
    #[must_use]
    pub fn from_reviews(reviews: &[Review]) -> Self {
        let total_reviews = reviews.len();
        let mut distribution = [0usize; 5];
        let mut sum = 0u64;

        for r in reviews {
            if r.rating >= 1 && r.rating <= 5 {
                distribution[(r.rating - 1) as usize] += 1;
                sum += r.rating as u64;
            }
        }

        let average_rating = if total_reviews > 0 {
            sum as f64 / total_reviews as f64
        } else {
            0.0
        };

        Self {
            total_reviews,
            average_rating,
            distribution,
        }
    }
}

/// Review store for a single package.
pub struct ReviewStore {
    reviews: Vec<Review>,
}

impl ReviewStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            reviews: Vec::new(),
        }
    }

    /// Add a review.
    pub fn add(&mut self, review: Review) {
        self.reviews.push(review);
    }

    /// Get all reviews.
    #[must_use]
    pub fn reviews(&self) -> &[Review] {
        &self.reviews
    }

    /// Get review count.
    #[must_use]
    pub fn count(&self) -> usize {
        self.reviews.len()
    }

    /// Get computed statistics.
    #[must_use]
    pub fn stats(&self) -> ReviewStats {
        ReviewStats::from_reviews(&self.reviews)
    }

    /// Get recent reviews sorted by timestamp.
    #[must_use]
    pub fn recent(&self, limit: usize) -> Vec<&Review> {
        let mut sorted: Vec<&Review> = self.reviews.iter().collect();
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        sorted.truncate(limit);
        sorted
    }
}

impl Default for ReviewStore {
    fn default() -> Self {
        Self::new()
    }
}
