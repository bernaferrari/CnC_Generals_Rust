// Default vertex shader for GameEngineDevice W3D system
#version 330 core

layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aTexCoord;

out vec3 FragPos;
out vec3 Normal;
out vec2 TexCoord;

uniform mat4 u_mvp_matrix;
uniform mat4 u_model_matrix;
uniform mat4 u_normal_matrix;

void main()
{
    FragPos = vec3(u_model_matrix * vec4(aPos, 1.0));
    Normal = mat3(u_normal_matrix) * aNormal;
    TexCoord = aTexCoord;
    
    gl_Position = u_mvp_matrix * vec4(aPos, 1.0);
}