$( document ).ready(function() {
    console.log( "ready!" );

    initVideoStreamByUrl("/api/dat/video0.jpg");
    initVideoStreamByUrl("/api/dat/video1.jpg");
    initVideoStreamByUrl("/api/dat/video2.jpg");

    getStats();
    
    // Initialize pump control UI
    initPumpControls();
});

function getStats(){
    $.ajax({
        url: "/api/stats",
        error: function(){
            getStats();
                //do something depressing
        },
        success: function(data){
            getStats();
            console.log(data);
            if(data !== undefined){
                $("#pm25_stat").html(data["pm25"]);
                $("#pm10_stat").html(data["pm10"]);
                $("#co2_stat").html(data["co2"]);
                $("#tvoc_stat").html(data["tvoc"]);
                $("#temp_stat").html(data["temp"]);
                $("#hum_stat").html(data["hum"]);
                
                // pH sensor data
                if(data["ph_status"]) {
                    var phStatus = data["ph_status"];
                    
                    // Display current pH value
                    if(phStatus.current_ph) {
                        $("#ph_stat").html(phStatus.current_ph.toFixed(2));
                        
                        // Color code based on alert level
                        if(phStatus.alert_level === "Critical") {
                            $("#ph_stat").addClass("text-danger").removeClass("text-warning text-success");
                            $("#ph_alert").html("⚠️ Critical").addClass("text-danger");
                        } else if(phStatus.alert_level === "Warning") {
                            $("#ph_stat").addClass("text-warning").removeClass("text-danger text-success");
                            $("#ph_alert").html("⚠️ Warning").addClass("text-warning");
                        } else {
                            $("#ph_stat").addClass("text-success").removeClass("text-danger text-warning");
                            $("#ph_alert").html("✓ Normal").addClass("text-success");
                        }
                    } else {
                        $("#ph_stat").html("N/A");
                    }
                    
                    // Display trend
                    if(phStatus.trend === "Rising") {
                        $("#ph_trend").html("↑ Rising");
                    } else if(phStatus.trend === "Falling") {
                        $("#ph_trend").html("↓ Falling");
                    } else {
                        $("#ph_trend").html("→ Stable");
                    }
                    
                    // Display adjustment suggestion
                    if(phStatus.adjustment_suggestion) {
                        $("#ph_suggestion").html(phStatus.adjustment_suggestion);
                    }
                } else if(data["ph"]) {
                    // Fallback to simple pH value
                    $("#ph_stat").html(data["ph"]);
                }
            }

        }
    });
}

function initVideoStreamByUrl(url){
    $.ajax({
        url: url,
        type:'HEAD',
        error: function(){
                //do something depressing
        },
        success: function(){
           

            let id = addCanvas();
            convertImageToCanvas(url, id);

        }
    });
}

function addCanvas(){
    var id = makeid(20);
    var cc = "<div class='col-xl-6 col-md-12 mb-4'>\
                <div class='card border-left-gray-800 shadow h-100 py-2'>\
                    <div class='card-body'>\
                        <div class='row no-gutters align-items-center'>\
                        <canvas style='width: 100%;' id='"+id+"'></canvas>\
                        </div>\
                    </div>\
                </div>\
            </div>";
    $("#video_feeds_containers").append(cc);
    return id;
}

function convertImageToCanvas(url, canvas_id) {

	var canvas = document.getElementById(canvas_id);

    var image = new Image();
    image.src = url;
    image.onload = function() {
        canvas.width = this.width;
        canvas.height = this.height;
        canvas.getContext("2d").drawImage(this, 0, 0);
    };

	return canvas;
}

function makeid(length) {
    var result           = '';
    var characters       = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    var charactersLength = characters.length;
    for ( var i = 0; i < length; i++ ) {
      result += characters.charAt(Math.floor(Math.random() * 
 charactersLength));
   }
   return result;
}

// Pump Control Functions
function initPumpControls() {
    // Toggle photo cycle settings visibility
    $('#photoCycleSwitch').change(function() {
        if($(this).is(':checked')) {
            $('#photoCycleSettings').show();
        } else {
            $('#photoCycleSettings').hide();
        }
    });
    
    // Toggle safety pin settings visibility
    $('#safetyPinSwitch').change(function() {
        if($(this).is(':checked')) {
            $('#safetyPinSettings').show();
        } else {
            $('#safetyPinSettings').hide();
        }
    });
    
    // Load pump settings when modal opens
    $('#pumpControlModal').on('show.bs.modal', function() {
        loadPumpSettings();
    });
    
    // Save pump settings
    $('#savePumpSettings').click(function() {
        savePumpSettings();
    });
}

function loadPumpSettings() {
    $.ajax({
        url: "/api/pump/config",
        type: "GET",
        success: function(data) {
            if(data && data.pump_config) {
                var config = data.pump_config;
                $('#continuousModeSwitch').prop('checked', config.continuous_mode);
                $('#photoCycleSwitch').prop('checked', config.photo_cycle_enabled);
                $('#photoCycleStart').val(config.photo_cycle_start_hour);
                $('#photoCycleEnd').val(config.photo_cycle_end_hour);
                
                if(config.safety_gpio_pin !== null) {
                    $('#safetyPinSwitch').prop('checked', true);
                    $('#safetyGpioPin').val(config.safety_gpio_pin);
                    $('#safetyPinSettings').show();
                } else {
                    $('#safetyPinSwitch').prop('checked', false);
                    $('#safetyPinSettings').hide();
                }
                
                $('#runtimeLimit').val(config.pump_runtime_limit_seconds);
                $('#cooldownPeriod').val(config.pump_cooldown_seconds);
                
                if(config.photo_cycle_enabled) {
                    $('#photoCycleSettings').show();
                }
                
                updatePumpStatus();
            }
        },
        error: function(xhr, status, error) {
            console.error("Failed to load pump settings:", error);
            $('#pumpStatus').text('Failed to load settings');
        }
    });
}

function savePumpSettings() {
    var settings = {
        continuous_mode: $('#continuousModeSwitch').is(':checked'),
        photo_cycle_enabled: $('#photoCycleSwitch').is(':checked'),
        photo_cycle_start_hour: parseInt($('#photoCycleStart').val()),
        photo_cycle_end_hour: parseInt($('#photoCycleEnd').val()),
        safety_gpio_pin: $('#safetyPinSwitch').is(':checked') ? parseInt($('#safetyGpioPin').val()) : null,
        pump_runtime_limit_seconds: parseInt($('#runtimeLimit').val()),
        pump_cooldown_seconds: parseInt($('#cooldownPeriod').val())
    };
    
    $.ajax({
        url: "/api/pump/config",
        type: "POST",
        contentType: "application/json",
        data: JSON.stringify(settings),
        success: function(response) {
            $('#pumpStatus').text('Settings saved successfully!');
            setTimeout(function() {
                $('#pumpControlModal').modal('hide');
            }, 1500);
        },
        error: function(xhr, status, error) {
            console.error("Failed to save pump settings:", error);
            $('#pumpStatus').text('Failed to save settings: ' + error);
        }
    });
}

function updatePumpStatus() {
    $.ajax({
        url: "/api/pump/status",
        type: "GET",
        success: function(data) {
            var statusText = 'Mode: ';
            if(data.continuous_mode) {
                statusText += 'Continuous';
            } else {
                statusText += 'Sensor-based';
            }
            
            if(data.photo_cycle_enabled) {
                statusText += ' | Photo Cycle: ' + data.photo_cycle_start_hour + ':00 - ' + data.photo_cycle_end_hour + ':00';
            }
            
            if(data.safety_gpio_pin !== null) {
                statusText += ' | Safety Pin: GPIO' + data.safety_gpio_pin;
            }
            
            $('#pumpStatus').text(statusText);
        },
        error: function(xhr, status, error) {
            $('#pumpStatus').text('Unable to fetch pump status');
        }
    });
}