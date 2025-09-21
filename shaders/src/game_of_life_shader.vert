#version 450

layout (location = 0) in vec3 vertexPosition;

layout (location = 6) uniform mat4 mvp;

void main() {
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
