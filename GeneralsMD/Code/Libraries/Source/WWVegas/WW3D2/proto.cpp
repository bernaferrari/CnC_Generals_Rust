#include "proto.h"
#include "mesh.h"
#include "hmdldef.h"
#include "hlod.h"
#include "w3derr.h"

/*
** Global instances of the default loaders for the asset manager to install
*/
MeshLoaderClass		_MeshLoader;
HModelLoaderClass		_HModelLoader;


/*
** Prototype Classes
** These prototypes are the "built-in" ones for the W3D library.
*/
PrimitivePrototypeClass::PrimitivePrototypeClass(RenderObjClass * proto)			
{ 
	Proto = proto; 
	assert(Proto); 
	Proto->Add_Ref(); 
}
PrimitivePrototypeClass::~PrimitivePrototypeClass(void)						
{ 
	if (Proto) { 
		Proto->Release_Ref(); 
	}
}

const char * PrimitivePrototypeClass::Get_Name(void) const			
{ 
	return Proto->Get_Name(); 
}	

int PrimitivePrototypeClass::Get_Class_ID(void) const	
{ 
	return Proto->Class_ID(); 
}

RenderObjClass * PrimitivePrototypeClass::Create(void)					
{ 
	return (RenderObjClass *)( SET_REF_OWNER( Proto->Clone() ) ); 
}	


class HModelPrototypeClass : public W3DMPO, public PrototypeClass
{
	W3DMPO_GLUE(HModelPrototypeClass)
public:
	HModelPrototypeClass(HModelDefClass * def)				{ HModelDef = def; assert(HModelDef); }

	virtual const char *			Get_Name(void)	const			{ return HModelDef->Get_Name(); }	
	virtual int								Get_Class_ID(void) const	{ return RenderObjClass::CLASSID_HLOD; }
	virtual RenderObjClass *	Create(void)					{ return NEW_REF( HLodClass, (*HModelDef) ); }	
	virtual void							DeleteSelf()										{ delete this; }

	HModelDefClass *				HModelDef;

protected:
	virtual ~HModelPrototypeClass(void)							{ if (HModelDef) delete HModelDef; }						 

};


/***********************************************************************************************
 * MeshLoaderClass::Load -- reads in a mesh and creates a prototype for it                     *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   7/28/98    GTH : Created.                                                                 *
 *=============================================================================================*/
PrototypeClass * MeshLoaderClass::Load_W3D(ChunkLoadClass & cload)
{
	MeshClass * mesh = NEW_REF( MeshClass, () );

	if (mesh == NULL) {
		return NULL;
	}

	if (mesh->Load_W3D(cload) != WW3D_ERROR_OK) {

		// if the load failed, delete the mesh
		assert(mesh->Num_Refs() == 1);
		mesh->Release_Ref();
		return NULL;

	} else {

		// create the prototype and add it to the lists
		PrimitivePrototypeClass * newproto = W3DNEW PrimitivePrototypeClass(mesh);
		mesh->Release_Ref();
		return newproto;
	
	}
}


/***********************************************************************************************
 * HModelLoaderClass::Load -- reads in an hmodel and creates a prototype for it                *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   7/28/98    GTH : Created.                                                                 *
 *=============================================================================================*/
PrototypeClass * HModelLoaderClass::Load_W3D(ChunkLoadClass & cload)
{
	HModelDefClass * hdef = W3DNEW HModelDefClass;

	if (hdef == NULL) {
		return NULL;
	}

	if (hdef->Load_W3D(cload) != HModelDefClass::OK) {

		// load failed, delete the model and return an error
		delete hdef;
		return NULL;

	} else {
	
		// ok, accept this model! 
		HModelPrototypeClass * hproto = W3DNEW HModelPrototypeClass(hdef);
		return hproto;
	
	}
}

