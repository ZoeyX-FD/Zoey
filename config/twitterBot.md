# Twitter Bot Configuration Guide

## Configuration Files
- `twitter_config.json`: Main configuration file for Twitter bot behavior
- Default settings are used if no config file is provided or if `enabled: false`

## Usage
1. Copy `twitter_config.example.json` to `twitter_config.json`
2. Set `enabled: true` to use custom configuration
3. Modify settings as needed
4. Set `TWITTER_CONFIG_PATH` in your .env file

## Configuration Options

### Timeline Settings
```json
{
    "max_tweet_length": 280,        // Maximum characters per tweet
    "max_history_tweets": 5,        // Number of tweets to keep in context
    "home_timeline_fetch_count": 1, // Tweets to fetch from home timeline
    "mentions_fetch_count": 3       // Number of mentions to process
}
```

### Search Queries
Configure automated searches:
```json
{
    "search_queries": [
        {
            "query": "cooking AND (recipe OR chef OR food) lang:en",
            "max_results": 3,           // Tweets to fetch per search
            "interval_minutes": 120,     // How often to run search
            "enable": true              // Enable/disable this search
        }
    ]
}
```

### Topic Timelines
Follow specific Twitter topics:
```json
{
    "topic_timelines": [
        {
            "topic_id": "825047592286871552",  // Twitter's topic ID
            "topic_name": "Food & Dining",      // For reference only
            "max_results": 3,
            "interval_minutes": 120,
            "enable": true
        }
    ]
}
```

### Common Topic IDs
- Food & Dining: "825047592286871552"
- Cooking: "839544274442051584"
- Food Science: "847872878041219072"
- Recipes: "838544362957578240"
- Restaurants: "852262932607926272"

### Filter Settings
```json
{
    "search_languages": ["en"],     // Language filters
    "exclude_retweets": true,       // Skip retweets
    "exclude_replies": true,        // Skip replies
    "min_likes": 10,               // Minimum likes required
    "min_retweets": 3              // Minimum retweets required
}
```

### Time Intervals (in seconds)
```json
{
    "min_action_interval": 300,     // 5 minutes minimum between actions
    "max_action_interval": 900,     // 15 minutes maximum between actions
    "min_task_interval": 1800,      // 30 minutes minimum between tasks
    "max_task_interval": 3600       // 1 hour maximum between tasks
}
```

### Feature Toggles
```json
{
    "enable_likes": true,           // Enable auto-liking
    "enable_retweets": true,        // Enable auto-retweeting
    "enable_quotes": true,          // Enable quote tweets
    "enable_replies": true          // Enable replies
}
```

### Rate Limits
```json
{
    "max_tweets_per_hour": 2,       // Maximum tweets per hour
    "max_likes_per_hour": 5,        // Maximum likes per hour
    "max_retweets_per_hour": 3      // Maximum retweets per hour
}
``` 