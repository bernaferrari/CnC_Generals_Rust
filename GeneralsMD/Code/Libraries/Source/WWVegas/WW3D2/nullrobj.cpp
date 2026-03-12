#include "nullrobj.h"
#include "chunkio.h"

#include <string.h>


NullLoaderClass _NullLoader;




Null3DObjClass::Null3DObjClass(const char * name)																	
{
	strcpy(Name, name);
}

Null3DObjClass::Null3DObjClass(const Null3DObjClass & src)									
{
	strcpy(Name, src.Name);
}

Null3DObjClass & Null3DObjClass::operator = (const Null3DObjClass & that)				
{
	strcpy(Name, that.Name);

	RenderObjClass::operator = (that); return *this; 
}

int Null3DObjClass::Class_ID(void) const													
{ 
	return CLASSID_NULL; 
}

RenderObjClass * Null3DObjClass::Clone(void) const									
{ 
	return NEW_REF( Null3DObjClass, (*this)); 
}

void Null3DObjClass::Render(RenderInfoClass & rinfo)
{ 
}

void Null3DObjClass::Get_Obj_Space_Bounding_Sphere(SphereClass & sphere) const
{
   sphere.Center.Set(0,0,0);
	sphere.Radius = 0.1f;
}

void Null3DObjClass::Get_Obj_Space_Bounding_Box(AABoxClass & box) const
{
	box.Center.Set(0,0,0);
	box.Extent.Set(0.1f,0.1f,0.1f);
}

/*
** NullPrototypeClass
*/

NullPrototypeClass::NullPrototypeClass (void)
{
	// Note that the other members of the definition are uninitialized..
	// So don't rely on them if the name is "NULL".
	strcpy(Definition.Name, "NULL");
}

NullPrototypeClass::NullPrototypeClass (const W3dNullObjectStruct &null)
{
	Definition = null;
}


/*
** NullLoaderClass
*/

PrototypeClass * NullLoaderClass::Load_W3D (ChunkLoadClass &cload)
{
	W3dNullObjectStruct null;
	cload.Read(&null,sizeof(null));
	return W3DNEW NullPrototypeClass(null);
}