set -euxo pipefail
arduino-cli compile --profile feather --export-binaries --no-color --quiet thinkink
ssh pi@zero '
    set -euxo pipefail
    sudo usbreset "Feather RP2040"
    device_found=false
    while true; do
        while IFS= read -r -d '\'\'' acm_device; do
            if [ "$(udevadm info --query=property "$acm_device" | grep ID_MODEL=Feather_RP2040)" ]; then
                stty -F "$acm_device" 1200
                device_found=true
                break
            fi
        done < <(find /dev -name '\''ttyACM*'\'' -print0)

        if [ "$device_found" = true ]; then
            break
        fi

        sleep 1
    done
    while ! lsblk -o NAME,LABEL | grep -q "RPI-RP2"; do
        sleep 1
    done
    sudo mount /media/feather
'
scp thinkink/build/rp2040.rp2040.adafruit_feather/thinkink.ino.uf2 pi@zero:/media/feather/
