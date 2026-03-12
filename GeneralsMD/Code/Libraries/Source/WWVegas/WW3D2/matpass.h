#if defined(_MSC_VER)
#pragma once
#endif

#ifndef MATPASS_H
#define MATPASS_H

#ifndef REFCOUNT_H
#include "refcount.h"
#endif

#ifndef SHADER_H
#include "shader.h"
#endif

#ifndef WWDEBUG_H
#include "wwdebug.h"
#endif


class TextureClass;
class VertexMaterialClass;
class MeshModelClass;
class OBBoxClass;

/**
** MaterialPassClass
**
** This class wraps all of the data needed to describe an additional
** material pass for any object.  The motivation for this class is to
** implement certain types of special effects.  All data needed to 
** apply the pass should be generated procedurally.  Typically a 
** vertex processor will be used to generate any needed u-v's or vertex
** colors.  Alternatively, we could add the option to request to
** re-use the model's existing u-v's or vertex colors.
**
**
*/
class MaterialPassClass : public RefCountClass
{
public:

	MaterialPassClass(void);
	~MaterialPassClass(void);

	/// MW: Had to make this virtual so app can perform direct/custom D3D setup.
	virtual void	Install_Materials(void) const;
	virtual void	UnInstall_Materials(void) const { };	///< reset/cleanup D3D states

	void							Set_Texture(TextureClass * Texture,int stage = 0);
	void							Set_Shader(ShaderClass shader);
	void							Set_Material(VertexMaterialClass * mat);

	TextureClass *				Get_Texture(int stage = 0) const;
	VertexMaterialClass *	Get_Material(void) const;

	TextureClass *				Peek_Texture(int stage = 0) const;
	ShaderClass					Peek_Shader(void)	const							{ return Shader; }
	VertexMaterialClass *	Peek_Material(void) const						{ return Material; }	

	void							Set_Cull_Volume(OBBoxClass * volume)		{ CullVolume = volume; }
	OBBoxClass *				Get_Cull_Volume(void) const					{ return CullVolume; }
	
	void							Enable_On_Translucent_Meshes(bool onoff)	{ EnableOnTranslucentMeshes = onoff; }
	bool							Is_Enabled_On_Translucent_Meshes(void)		{ return EnableOnTranslucentMeshes; }

	static void					Enable_Per_Polygon_Culling(bool onoff)		{ EnablePerPolygonCulling = onoff; }
	static bool					Is_Per_Polygon_Culling_Enabled(void)		{ return EnablePerPolygonCulling; }

protected:
	
	enum { MAX_TEX_STAGES = 8 };

	TextureClass *				Texture[MAX_TEX_STAGES];
	ShaderClass					Shader;
	VertexMaterialClass *	Material;
	bool							EnableOnTranslucentMeshes;

	OBBoxClass *				CullVolume;
	static bool					EnablePerPolygonCulling;

};


inline TextureClass * MaterialPassClass::Peek_Texture(int stage) const
{	
	WWASSERT(stage >= 0);
	WWASSERT(stage < MAX_TEX_STAGES);
	return Texture[stage];
}


#endif // MATPASS_H
