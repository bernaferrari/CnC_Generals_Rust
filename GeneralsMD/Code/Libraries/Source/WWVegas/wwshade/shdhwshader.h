#ifndef SHDHWSHADER_H
#define SHDHWSHADER_H

#ifndef _D3D8_H_
#include <d3d8.h>
#endif

#ifndef __D3DX8_H__
#include <d3dx8.h>
#endif

#ifndef SHDHW_CONSTANTS_H
#include "shdhw_constants.h"
#endif


class RenderInfoClass;
class Vector4;

class ShdHWShader
{
public:
	ShdHWShader() : Shader(0) {}
	virtual ~ShdHWShader() {}

	DWORD Peek_Shader() const { return Shader; }

protected:

	void Shell_Run(char* cmd);

	void Preprocess_And_Assemble_Shader_From_File
	(
		char*				file_name,
		LPD3DXBUFFER*	constants,
		LPD3DXBUFFER*	shader_code
	);

	DWORD Shader;
};

class ShdHWVertexShader : public ShdHWShader
{
public:
	virtual ~ShdHWVertexShader();

	DWORD Create
	(
		char* file_name, 
		DWORD* vertex_shader_declaration
	);

	DWORD Create
	(
		DWORD* shader_code, 
		DWORD* vertex_shader_declaration
	);

	void Destroy();

	static bool	Is_Using_Hardware() { return Using_Hardware; }

	static void Light
	(
		RenderInfoClass&		rinfo,
		Vector4&					ambient,
		Vector4&					diffuse,
		Vector4&					specular
	);

	static void Light
	(
		RenderInfoClass&		rinfo,
		Vector4&					ambient,
		Vector4&					diffuse
	);

private:
	static bool Using_Hardware;
};

class ShdHWPixelShader : public ShdHWShader
{
public:
	virtual ~ShdHWPixelShader();

	DWORD Create(char* file_name);
	DWORD Create(DWORD* shader_code);

	void Destroy();
};

#endif
