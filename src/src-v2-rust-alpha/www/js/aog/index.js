$( document ).ready(function() {
    console.log( "ready!" );

    initVideoStreamByUrl("/api/dat/video0.jpg");
    initVideoStreamByUrl("/api/dat/video1.jpg");
    initVideoStreamByUrl("/api/dat/video2.jpg");


    getStats();

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