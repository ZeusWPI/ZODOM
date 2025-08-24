setInterval(checkForUpdate, 1000);

function checkForUpdate() {
    fetch("/current_song")
        .then((response) => response.json())
        .then((response) => {
            if (response !== currentSongId) {
                console.log("Reloading");
                location.reload();
            }
        })
}