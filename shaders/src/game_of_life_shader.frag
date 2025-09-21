#version 450

uniform sampler2D texture0;

layout (location = 0) uniform float pixelInverse;

layout (location = 0) out vec4 finalColor;

int cellValue(int dx, int dy) {
    vec2 st = (gl_FragCoord.xy + vec2(dx, dy)) * pixelInverse;

    return int(texture(texture0, st) == vec4(1.0));
}

void main() {
    int neighborCount = (
        0
        + cellValue( 0,  1)
        + cellValue( 1,  1)
        + cellValue( 1,  0)
        + cellValue( 1, -1)
        + cellValue( 0, -1)
        + cellValue(-1, -1)
        + cellValue(-1,  0)
        + cellValue(-1,  1)
    );

    if (neighborCount == 3 || bool(cellValue(0, 0)) && neighborCount == 2) {
        finalColor = vec4(1.0);
    } else {
        finalColor = vec4(vec3(0.0), 1.0);
    }
}
