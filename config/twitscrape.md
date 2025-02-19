# Twitter Analysis Configuration

## Overview
This configuration file controls the behavior of the Twitter analysis system.

## Parameters

### Basic Settings
- `targets`: List of Twitter usernames to analyze
- `tweets_per_user`: Number of tweets to fetch per user
- `wait_time`: Delay between requests in seconds
- `include_replies`: Whether to include reply tweets
- `save_to_file`: Whether to save results to files
- `custom_keywords`: List of keywords to track

### Analysis Settings
- `sentiment_threshold`: Minimum score for sentiment classification
- `min_engagement`: Minimum engagement for tweet analysis
- `exclude_retweets`: Whether to exclude retweets
- `language`: Target language for tweets
- `date_range_days`: How many days of tweets to analyze

### Categories
Predefined categories with associated keywords for classification:
- DeFi
- Layer2
- NFT
- Infrastructure
- Trading

### Metrics
Configure which metrics to track:
- Followers
- Engagement
- Sentiment
- Topics
- Mentions
- URLs

### Export Options
- Supported formats: JSON, CSV

### Notifications
Alert settings for:
- High engagement tweets
- Sentiment changes
- Topic surges

## Usage
1. Modify `twitter_analysis.json` as needed
2. Run with: `cargo run --example twitter_follow_extract`
3. Or specify target: `cargo run --example twitter_follow_extract username`

## Environment Variables
Required:
- `TWITTER_COOKIE_STRING`: Twitter authentication cookie
Optional:
- `AUTO_FOLLOW`: Set to "true" to auto-follow users 

# Quick analysis
cargo run --example twitter_follow_extract username 300 7 ( this tells the program to fetch max 300 of the latest tweets from the user in the last 7 days)

# Detailed analysis with AI
cargo run --example twitter_follow_extract username 300 7 ai ( this will use the AI to analyze the 300 latest tweets from the user in the last 7 days and provide a detailed analysis)

# Multiple users from file
cargo run --example twitter_follow_extract -- --file users.txt 50 1 ai ( this will use the AI to analyze the tweets and provide a detailed analysis for each user in the users.txt on root directory folder)