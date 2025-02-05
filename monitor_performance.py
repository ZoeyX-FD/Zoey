import psutil
import time
from datetime import datetime
import os
import csv
from pathlib import Path

def should_show_process(proc_info):
    status = proc_info.get('status', '')
    cpu = proc_info.get('cpu_percent', 0)
    mem = proc_info.get('memory_percent', 0)
    
    # Show if:
    # 1. Process is running or sleeping (not stopped)
    # 2. Using CPU or significant memory
    # 3. Started in last 5 minutes
    return (
        status not in ['stopped', 'dead', 'zombie'] or
        cpu > 0.1 or
        mem > 0.1
    )

def get_rig_processes():
    rig_processes = []
    rig_path = "/root/RIGnew"
    
    for proc in psutil.process_iter(['pid', 'name', 'cmdline', 'cpu_percent', 'memory_percent', 'status', 'create_time']):
        try:
            cmdline = proc.info['cmdline']
            name = proc.info['name'].lower()
            
            # Include process if it:
            # 1. Has RIGnew in its command line
            # 2. Is a Python process
            # 3. Has 'rig' in its name
            # 4. Is running from the RIG directory
            # 5. Is a Rust/Cargo process
            # 6. Is a crypto-agents process
            if any((
                cmdline and any(rig_path in cmd for cmd in cmdline),
                'python' in name,
                'rig' in name.lower(),
                'rust' in name.lower(),
                'cargo' in name.lower(),
                cmdline and any('crypto-agents' in cmd.lower() for cmd in cmdline),
                cmdline and any('monitor' in cmd.lower() for cmd in cmdline),
                cmdline and any('.rs' in cmd.lower() for cmd in cmdline)
            )):
                if should_show_process(proc.info):
                    rig_processes.append(proc)
                
            # Also check for specific executables
            if cmdline:
                executable = cmdline[0].lower() if cmdline else ""
                if any((
                    'target/release' in executable,
                    'target/debug' in executable,
                    'crypto-agents' in executable
                )):
                    if should_show_process(proc.info):
                        rig_processes.append(proc)
                    
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
            
    # Remove duplicates based on PID
    seen_pids = set()
    unique_processes = []
    for proc in rig_processes:
        if proc.pid not in seen_pids:
            seen_pids.add(proc.pid)
            unique_processes.append(proc)
            
    return unique_processes

def format_size(bytes):
    for unit in ['B', 'KB', 'MB', 'GB']:
        if bytes < 1024:
            return f"{bytes:.2f}{unit}"
        bytes /= 1024
    return f"{bytes:.2f}TB"

def monitor_performance():
    # Create directory for logs
    log_dir = Path("data/performance_logs")
    log_dir.mkdir(parents=True, exist_ok=True)
    
    # Create CSV file for logging
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    log_file = log_dir / f"performance_log_{timestamp}.csv"
    
    print(f"🔍 Starting performance monitoring...")
    print(f"📝 Logging to: {log_file}")
    print("\n" + "=" * 120)
    
    with open(log_file, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['Timestamp', 'Process', 'PID', 'CPU%', 'Memory%', 'Memory_Used', 'Threads', 'Status'])
        
        start_time = time.time()
        interval = 60  # Log every minute
        
        try:
            while True:
                current_time = time.time()
                elapsed_hours = (current_time - start_time) / 3600
                
                processes = get_rig_processes()
                timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                
                # Clear screen and show current stats
                os.system('clear')
                print(f"🤖 RIGnew Performance Monitor")
                print(f"⏱️  Running for: {elapsed_hours:.2f} hours")
                print(f"🕒 Last update: {timestamp}")
                print("=" * 120)
                print(f"{'Process':<40} {'PID':<8} {'CPU%':<8} {'Memory%':<10} {'Memory Used':<12} {'Threads':<8} {'Status':<10}")
                print("-" * 120)
                
                total_cpu = 0
                total_memory = 0
                active_processes = 0
                
                for proc in processes:
                    try:
                        with proc.oneshot():
                            cpu_percent = proc.cpu_percent()
                            memory_percent = proc.memory_percent()
                            memory_used = format_size(proc.memory_info().rss)
                            threads = proc.num_threads()
                            status = proc.status()
                            
                            # Get process name with more detail
                            if proc.cmdline():
                                name = ' '.join(proc.cmdline())
                                # Keep important parts when truncating
                                if len(name) > 40:
                                    # Try to keep the executable name
                                    parts = name.split('/')
                                    if len(parts) > 1:
                                        name = ".../" + '/'.join(parts[-2:])
                                    if len(name) > 40:
                                        name = name[:37] + "..."
                            else:
                                name = proc.name()
                            
                            total_cpu += cpu_percent
                            total_memory += memory_percent
                            
                            # Write to CSV
                            writer.writerow([
                                timestamp, name, proc.pid, cpu_percent,
                                memory_percent, memory_used, threads, status
                            ])
                            
                            # Color code the status
                            if status == 'running':
                                status_color = '\033[92m'  # Green
                            elif status == 'sleeping':
                                status_color = '\033[94m'  # Blue
                            else:
                                status_color = '\033[0m'   # Default
                            
                            # Show process info with colored status
                            print(f"{name:<40} {proc.pid:<8} {cpu_percent:>6.1f}% {memory_percent:>8.1f}% {memory_used:>11} {threads:>7} {status_color}{status:<10}\033[0m")
                            if cpu_percent > 0 or memory_percent > 0:
                                active_processes += 1
                    
                    except (psutil.NoSuchProcess, psutil.AccessDenied):
                        continue
                
                print("-" * 120)
                print(f"Active Processes: {active_processes}")
                print(f"Total CPU Usage: {total_cpu:.1f}%")
                print(f"Total Memory Usage: {total_memory:.1f}%")
                print(f"\n📊 Performance data is being logged to: {log_file}")
                print(f"⏱️  Next update in {interval} seconds...")
                
                # Flush the CSV file to ensure data is written
                f.flush()
                
                time.sleep(interval)
                
        except KeyboardInterrupt:
            print("\n\n✋ Monitoring stopped by user")
            print(f"📊 Performance log saved to: {log_file}")

if __name__ == "__main__":
    monitor_performance() 