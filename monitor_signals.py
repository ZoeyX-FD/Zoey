import csv
import time
from datetime import datetime
import os
from colorama import init, Fore, Style

# Initialize colorama for Windows compatibility
init()

def clear_screen():
    os.system('cls' if os.name == 'nt' else 'clear')

def format_price(price):
    if price == 0:
        return "No data"
    elif price < 0.01:
        return f"${price:.8f}"
    elif price < 1:
        return f"${price:.4f}"
    else:
        return f"${price:.2f}"

def format_market_cap(market_cap):
    if market_cap == 0:
        return "No data"
    elif market_cap >= 1_000_000_000:  # Billions
        return f"${market_cap/1_000_000_000:.2f}B"
    elif market_cap >= 1_000_000:  # Millions
        return f"${market_cap/1_000_000:.2f}M"
    else:  # Thousands
        return f"${market_cap/1_000:.2f}K"

def format_percentage(percentage):
    if percentage == 0:
        return "N/A"
    elif percentage > 0:
        return f"+{percentage:.2f}%"
    else:
        return f"{percentage:.2f}%"

def read_and_display_signals():
    try:
        with open('data/market_analysis/analysis_results.csv', 'r') as file:
            reader = csv.DictReader(file)
            signals = []
            
            for row in reader:
                try:
                    price = float(row.get('price', 0))
                    change_24h = float(row.get('change_24h', 0))
                except (ValueError, TypeError):
                    price = 0
                    change_24h = 0
                    
                signals.append({
                    'timestamp': row.get('timestamp', ''),
                    'name': row.get('name', ''),
                    'symbol': row.get('symbol', ''),
                    'price': price,
                    'change_24h': change_24h,
                    'recommendation': row.get('recommendation', '')
                })
            
            # Sort by timestamp (most recent first)
            signals.sort(key=lambda x: x['timestamp'], reverse=True)
            
            # Get unique latest signals (keep only the most recent signal for each token)
            seen_tokens = set()
            latest_signals = []
            for signal in signals:
                if signal['name'] not in seen_tokens:
                    latest_signals.append(signal)
                    seen_tokens.add(signal['name'])
            
            # Display signals
            clear_screen()
            
            print("\n🤖 Latest Trading Signals - 2025-02-02")
            print(f"📊 Last Update: 12:03:05")
            print("=" * 100)
            print(f"{'Token':<20} {'Price':<15} {'Signal':<12} {'1h Change':<12} {'Time':<10}")
            print("-" * 100)
            
            for signal in latest_signals[:20]:
                price_str = format_price(signal['price'])
                
                # Format 24h change with color
                if signal['change_24h'] > 0:
                    change_str = f"{Fore.GREEN}+{signal['change_24h']:.2f}%{Style.RESET_ALL}"
                elif signal['change_24h'] < 0:
                    change_str = f"{Fore.RED}{signal['change_24h']:.2f}%{Style.RESET_ALL}"
                else:
                    change_str = f"{Style.RESET_ALL}0.00%{Style.RESET_ALL}"
                
                # Color coding for recommendation
                if signal['recommendation'] == 'NEW LISTING':
                    color = Fore.CYAN
                    change_str = f"{Fore.CYAN}NEW{Style.RESET_ALL}"
                elif signal['recommendation'] == 'BUY':
                    color = Fore.GREEN
                elif signal['recommendation'] == 'SELL':
                    color = Fore.RED
                else:
                    color = Style.RESET_ALL
                
                print(f"{color}{signal['name']:<20} {price_str:<15} {signal['recommendation']:<12}{Style.RESET_ALL} {change_str:<15} 12:03:05")
            
            print("\n" + "=" * 100)
            
    except FileNotFoundError:
        print("Error: analysis_results.csv not found")
    except Exception as e:
        print(f"Error: {str(e)}")

def main():
    while True:
        read_and_display_signals()
        time.sleep(5)  # Update every 5 seconds

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nMonitoring stopped by user") 