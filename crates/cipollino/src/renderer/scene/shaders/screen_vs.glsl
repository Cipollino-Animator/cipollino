
#version 100 

attribute vec2 aPos;
attribute vec2 aUv;

varying vec2 pUv;

void main() {
    gl_Position = vec4(aPos, 0.0, 1.0);
    pUv = aUv;
} 
