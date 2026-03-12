#ifndef SHD7BUMPSPEC_H
#define SHD7BUMPSPEC_H

#ifndef SHDINTERFACE_H
#include "shdinterface.h"
#endif

#ifndef SHDHWSHADER_H
#include "shdhwshader.h"
#endif


class Shd7BumpSpecClass : public ShdInterfaceClass
{
public:
	Shd7BumpSpecClass(const ShdDefClass* def);
	virtual ~Shd7BumpSpecClass();
	
	static void Init();
	static void Shutdown();

	virtual int							Get_Pass_Count() { return 2; }

	virtual int							Get_Texture_Count() const { return 2; }
	virtual TextureClass*			Peek_Texture(int idx) const { return idx==0 ? Texture : NormalMap; }

	virtual void						Apply_Shared(int cur_pass, RenderInfoClass& rinfo);
	virtual void						Apply_Instance(int cur_pass, RenderInfoClass& rinfo);

	virtual unsigned					Get_Vertex_Stream_Count() const;
	virtual unsigned					Get_Vertex_Size(unsigned stream) const;
	virtual bool						Use_HW_Vertex_Processing() const { return Pass_0_Vertex_Shader.Is_Using_Hardware(); }
	virtual void						Copy_Vertex_Stream
	(
		unsigned stream, 
		void* dest_buffer, 
		const VertexStreamStruct& vss, 
		unsigned vertex_count
	);

protected:

	static ShdHWVertexShader		Pass_0_Vertex_Shader;
	static ShdHWVertexShader		Pass_1_Vertex_Shader;

	static Matrix4x4					View_Projection_Matrix;

	TextureClass*			Texture;
	TextureClass*			NormalMap;

	Vector4					Ambient;
	Vector4					Diffuse;
	Vector4					Specular;
	Vector4					Bumpiness;
};

#endif // SHD7BUMPSPEC_H
