#include "polyinfo.h"
#include "texture.h"
#include "vertmaterial.h"
#include "shader.h"

void PolygonInfoClass::Set_Texture(TextureClass *texture) 
{
	if(texture)
		texture->Add_Ref();
	if(Texture)
		Texture->Release_Ref();
	Texture = texture;
}
void PolygonInfoClass::Set_Vertex_Material(VertexMaterialClass *vertexMaterial) 
{
	if(vertexMaterial)
		vertexMaterial->Add_Ref();
	if(VertexMaterial)
		VertexMaterial->Release_Ref();

	VertexMaterial = vertexMaterial;
}

void PolygonInfoClass::Set_Shader(ShaderClass *shader)
{
	if(Shader)
		delete Shader;

	// todo : update for refcounted shaders
	Shader = W3DNEW ShaderClass(* shader);
}


PolygonInfoClass::~PolygonInfoClass()
{
	if(Texture)
		Texture->Release_Ref();
	if(VertexMaterial)
		VertexMaterial->Release_Ref();

	// todo : update for refcounted shaders	
	if(Shader)
		delete Shader;
}


