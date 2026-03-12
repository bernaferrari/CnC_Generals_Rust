#ifndef W3DDESC_H
#define W3DDESC_H

#include "always.h"
#include <Max.h>

/*****************************************************************************
*
*  Class descriptors provide the system with information about the plug-in 
*  classes in the DLL.  
*
*****************************************************************************/
#define W3D_EXPORTER_CLASS_ID Class_ID(0x54d412df, 0x41466ae8)

class W3dClassDesc : public ClassDesc
{
public:
	void *			Create(BOOL);
	int				IsPublic();
	const TCHAR *	ClassName();
	SClass_ID		SuperClassID(); 
	Class_ID			ClassID();
	const TCHAR *	Category();
};


#endif