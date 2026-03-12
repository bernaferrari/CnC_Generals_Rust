// FILE: ClientUpdateModule.h /////////////////////////////////////////////////////////////////////////////////
// Author: 
// Desc:	 
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __ClientUpdateModule_H_
#define __ClientUpdateModule_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include <stdlib.h>

#include "Common/Module.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////

// TYPES //////////////////////////////////////////////////////////////////////////////////////////

typedef ModuleData ClientUpdateModuleData;

//-------------------------------------------------------------------------------------------------
/** DRAWABLE CLIENT UPDATE MODULE base class */
//-------------------------------------------------------------------------------------------------
class ClientUpdateModule : public DrawableModule
{

	MEMORY_POOL_GLUE_ABC( ClientUpdateModule )

public:

	ClientUpdateModule( Thing *thing, const ModuleData* moduleData );
	static ModuleType getModuleType() { return MODULETYPE_CLIENT_UPDATE; }
	static Int getInterfaceMask() { return MODULEINTERFACE_CLIENT_UPDATE; }

	virtual void clientUpdate() = 0;

};
inline ClientUpdateModule::ClientUpdateModule( Thing *thing, const ModuleData* moduleData ) : DrawableModule( thing, moduleData ) { }
inline ClientUpdateModule::~ClientUpdateModule() { }
//-------------------------------------------------------------------------------------------------


#endif // __ClientUpdateModule_H_

