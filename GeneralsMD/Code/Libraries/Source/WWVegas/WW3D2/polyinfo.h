#ifndef POLYINFO_H
#define POLYINFO_H

#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

class TextureClass;
class VertexMaterialClass;
class ShaderClass;


/**
**	PolyInfo.h
**
**	This class is a simple storage class that keeps track of a texture, vertex material
**	and a shader.
**
*/
class PolygonInfoClass
{

public:

	TextureClass			* Peek_Texture()				const	{ return Texture; }
	VertexMaterialClass	* Peek_Vertex_Material()	const { return VertexMaterial; }
	ShaderClass				* Peek_Shader()				const { return Shader; }

	void Set_Texture(TextureClass *texture);
	void Set_Vertex_Material(VertexMaterialClass *vertexMaterial);
	void Set_Shader(ShaderClass *shader);

	void Set(TextureClass *texture, VertexMaterialClass *vertexMaterial, ShaderClass *shader)
	{
		Set_Texture(texture);
		Set_Vertex_Material(vertexMaterial);
		Set_Shader(shader);
	}

	PolygonInfoClass(TextureClass *texture = 0, VertexMaterialClass *vertexMaterial = 0, ShaderClass *shader = 0)
	: Texture(0), VertexMaterial(0), Shader(0)
	{
		Set(texture, vertexMaterial, shader);
	}

	~PolygonInfoClass();


protected:

	TextureClass *Texture;
	VertexMaterialClass *VertexMaterial;
	ShaderClass *Shader;
};

#endif

