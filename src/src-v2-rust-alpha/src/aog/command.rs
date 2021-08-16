use crate::aog;

pub fn run(command: String){
    if command.starts_with("cls") || command.starts_with("clear"){
        aog::cls();
    }

    if command == "gpio status".to_string(){
        aog::gpio_status::init();
    }

    // 0-21
    if command.starts_with("gpio_on"){

    }

    if command.starts_with("gpio_off"){
        
    }

    if command == "help".to_string(){
        println!("clear/cls:                clears screen");
        println!("help [command]:           shows help");
    }


}