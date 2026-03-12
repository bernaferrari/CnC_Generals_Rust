#ifndef SHDSUBMESH_H
#define SHDSUBMESH_H

#include "always.h"
#include "meshgeometry.h"
#include "shdinterface.h"
#include "dx8wrapper.h"
#include "sharebuf.h"

class DecalGeneratorClass;
class ShdInterfaceClass;
class RenderInfoClass;
class MeshModelClass;
class HTreeClass;

/**
** ShdSubMeshClass - A single ShdMeshClass instance will contain one or more ShdSubMeshClass.  
** The "sub-mesh" is a collection of polygons who all use the same shader.  
*/
class ShdSubMeshClass : public MeshGeometryClass
{
public:	
	W3DMPO_GLUE(ShdSubMeshClass);
	
	ShdSubMeshClass(void);
	ShdSubMeshClass(const ShdSubMeshClass & that);
	~ShdSubMeshClass(void);
	
	ShdSubMeshClass & operator = (const ShdSubMeshClass & that);
	void							Reset(int polycount,int vertcount);

//	unsigned Get_Vertex_Stream_Count();
//	unsigned Get_Vertex_Size(unsigned stream);
//	void Copy_Vertex_Stream(unsigned stream, void* buffer);

	/////////////////////////////////////////////////////////////////////////////////////
	// Material interface, All of these functions call through to the current
	// material decription.
	/////////////////////////////////////////////////////////////////////////////////////
	void							Set_Shader(ShdInterfaceClass * shader)										{ REF_PTR_SET(Shader,shader); } 
	ShdInterfaceClass *		Peek_Shader(void) const															{ return Shader; }

	/////////////////////////////////////////////////////////////////////////////////////
	//	Decal interface
	/////////////////////////////////////////////////////////////////////////////////////
	void							Create_Decal(DecalGeneratorClass * generator);
	void							Delete_Decal(uint32 decal_id);

	void							Init_From_Legacy_Mesh_Model(MeshModelClass* model,int first_polygon);

	/////////////////////////////////////////////////////////////////////////////////////
	// Load from a W3D File
	/////////////////////////////////////////////////////////////////////////////////////
	WW3DErrorType				Load_W3D(ChunkLoadClass& cload);

	const Vector2*				Get_UV_Array(unsigned stage) { return UV[stage] ? UV[stage]->Get_Array() : NULL; }
	const unsigned*			Get_Diffuse_Array() { return Diffuse ? Diffuse->Get_Array() : NULL; }
	const Vector3*				Get_Tangent_Basis_S_Array() { return S ? S->Get_Array() : NULL; }
	const Vector3*				Get_Tangent_Basis_T_Array() { return T ? T->Get_Array() : NULL; }
	const Vector3*				Get_Tangent_Basis_SxT_Array() { return SxT ? SxT->Get_Array() : NULL; }

	// First and last visible polygon. Default is both zero, which means that all polygons
	// are visible.
	int							Get_First_Visible_Polygon() const { return FirstVisiblePolygon; }
	int							Get_Visible_Polygon_Count() const { return VisiblePolygonCount; }
	bool							Is_Sorting() const { return Sorting; }

	void							Get_Deformed_Vertices(Vector3 *dst_vert, Vector3 *dst_norm, const HTreeClass* htree);
	void							Get_Deformed_Vertices(Vector3 *dst_vert, const HTreeClass* htree);

protected:
	ShareBufferClass<Vector2> *						UV[MAX_TEXTURE_STAGES];
	ShareBufferClass<unsigned> *						Diffuse;

	ShareBufferClass<Vector3>*							S;
	ShareBufferClass<Vector3>*							T;
	ShareBufferClass<Vector3>*							SxT;

	int														FirstVisiblePolygon;
	int														VisiblePolygonCount;
	bool											Sorting;

	// MeshClass will set this for skins so that they can get the bone transforms
//	void							Set_HTree(const HTreeClass * htree);

protected:
	// functions to compute the deformed vertices of skins.
	// Destination pointers MUST point to arrays large enough to hold all vertices
//	void get_deformed_vertices(Vector3 *dst_vert, Vector3 *dst_norm, const HTreeClass * htree);
//	void get_deformed_vertices(Vector3 *dst_vert, const HTreeClass * htree);
//	void get_deformed_screenspace_vertices(Vector4 *dst_vert,const RenderInfoClass & rinfo,const Matrix3D & mesh_tm,const HTreeClass * htree);

	// loading
	WW3DErrorType read_chunks(ChunkLoadClass& cload);
	WW3DErrorType read_vertices(ChunkLoadClass& cload);
	WW3DErrorType read_vertex_normals(ChunkLoadClass& cload);

	WW3DErrorType read_uv0(ChunkLoadClass& cload);
	WW3DErrorType read_uv1(ChunkLoadClass& cload);

	WW3DErrorType read_tangent_basis_s(ChunkLoadClass& cload);
	WW3DErrorType read_tangent_basis_t(ChunkLoadClass& cload);
	WW3DErrorType read_tangent_basis_sxt(ChunkLoadClass& cload);
	WW3DErrorType read_vertex_influences(ChunkLoadClass& cload);

	WW3DErrorType read_triangles(ChunkLoadClass& cload);
	WW3DErrorType read_vertex_shade_indices(ChunkLoadClass& cload);
	WW3DErrorType read_shader(ChunkLoadClass& cload);

//	WW3DErrorType read_texcoords(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_materials(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_v2_materials(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_v3_materials(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_per_tri_materials(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_vertex_colors(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_material_info(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_shaders(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_vertex_materials(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_textures(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_material_pass(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_vertex_material_ids(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_shader_ids(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_scg(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_dig(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_dcg(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_texture_stage(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_texture_ids(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_stage_texcoords(ChunkLoadClass & cload,ShdMeshLoadContextClass * context);
//	WW3DErrorType read_per_face_texcoord_ids (ChunkLoadClass &cload, ShdMeshLoadContextClass *context);
//	WW3DErrorType read_prelit_material (ChunkLoadClass &cload, ShdMeshLoadContextClass *context);

	ShdInterfaceClass *									Shader;
};

#endif //SHDSUBMESH_H
