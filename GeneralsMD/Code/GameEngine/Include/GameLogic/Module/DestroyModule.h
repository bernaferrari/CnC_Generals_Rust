// FILE: DestroyModule.h /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2001
// Desc:	 
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DestroyModule_H_
#define __DestroyModule_H_

#include "Common/Module.h"
#include "GameLogic/Module/BehaviorModule.h"

//-------------------------------------------------------------------------------------------------
/** OBJECT DESTROY MODULE base class */
//-------------------------------------------------------------------------------------------------
class DestroyModuleInterface
{
public:
	virtual void onDestroy() = 0;
};

//-------------------------------------------------------------------------------------------------
class DestroyModule : public BehaviorModule, public DestroyModuleInterface
{

	MEMORY_POOL_GLUE_ABC( DestroyModule )
	MAKE_STANDARD_MODULE_MACRO_ABC( DestroyModule )

public:

	DestroyModule( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	static Int getInterfaceMask() { return MODULEINTERFACE_DESTROY; }

	// BehaviorModule
	virtual DestroyModuleInterface* getDestroy() { return this; }

	virtual void onDestroy() = 0;

protected:

};

#endif
