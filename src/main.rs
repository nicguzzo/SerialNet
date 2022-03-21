use serialport::{available_ports, SerialPortType};
use std::io::{self, Write, Read};
use std::time::Duration;
use clap::Parser;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::error::Error;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value = "")]
    serial_port: String,
    #[clap(short,long )]
    list_ports: bool,
    #[clap(short, long, default_value = "250000")]
    bauds:u32,
    #[clap(short, long, default_value = "0.0.0.0")]
    ip:String,
    #[clap(short, long, default_value = "9090")]
    net_port:u32
}

fn list_ports() {
    match available_ports() {
        Ok(ports) => {
            match ports.len() {
                0 => println!("No ports found."),
                1 => println!("Found 1 port:"),
                n => println!("Found {} ports:", n),
            };
            for p in ports {
                println!("  {}", p.port_name);
                match p.port_type {
                    SerialPortType::UsbPort(info) => {
                        println!("    Type: USB");
                        println!("    VID:{:04x} PID:{:04x}", info.vid, info.pid);
                        println!(
                            "     Serial Number: {}",
                            info.serial_number.as_ref().map_or("", String::as_str)
                        );
                        println!(
                            "      Manufacturer: {}",
                            info.manufacturer.as_ref().map_or("", String::as_str)
                        );
                        println!(
                            "           Product: {}",
                            info.product.as_ref().map_or("", String::as_str)
                        );
                    }
                    SerialPortType::BluetoothPort => {
                        println!("    Type: Bluetooth");
                    }
                    SerialPortType::PciPort => {
                        println!("    Type: PCI");
                    }
                    SerialPortType::Unknown => {
                        println!("    Type: Unknown");
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
            eprintln!("Error listing serial ports");
        }
    }
}
#[tokio::main]
async fn main()-> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.list_ports  {
        list_ports();
    }
    let port_name=if args.serial_port==""{
        serialport::available_ports().expect("No serial port")[0].port_name.clone()
    }else{
        args.serial_port
    };
    let mut n_clients=0;
    
    let listener = TcpListener::bind(format!("{}:{}",args.ip,args.net_port)).await?;
    println!("Using serial port {} at {}",port_name,args.bauds);
    println!("listening at {}:{}",&args.ip,args.net_port);
    loop {
        
        let (socket, _) = listener.accept().await?;
        let (mut rd, mut wr) = tokio::io::split(socket);
        if n_clients>0{
            println!("only one client per serial port");
            tokio::spawn(async move {
                wr.write_all(b"only one client per serial port");
            });

        }else{
            n_clients+=1;            
            let port=port_name.clone();
            println!("Connection established!");

            let port = serialport::new(port, args.bauds)
            .timeout(Duration::from_millis(10))
            .open();

            match port {
                Ok(mut port) => {
                    let mut clone = port.try_clone().expect("Failed to clone");
                    let mut serial_buf: Vec<u8> = vec![0; 1024];
                    tokio::spawn(async move {
                        loop {
                            match port.read(serial_buf.as_mut_slice()) {
                                Ok(n) => {                            
                                    if let Err(e) = wr.write_all(&serial_buf[0..n]).await {
                                        eprintln!("failed to write to socket; err = {:?}", e);
                                        //return;
                                    }
                                },
                                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                                Err(e) => eprintln!("{:?}", e),
                            }
                        }
                    });
                    loop {
                        let mut net_buf = [0; 1024];
                        let n = rd.read(&mut net_buf).await?;
                        if n == 0 {
                            break;
                        }
                        clone.write(&net_buf[..n]);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to open \"{}\". Error: {}", port_name, e);
                    ::std::process::exit(1);
                }
            }
        }
    }   
}
