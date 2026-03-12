// FILE: CreateModule.h /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2001
// Desc:	 
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __CreateModule_H_
#define __CreateModule_H_

#include "Common/Module.h"
#include "GameLogic/Module/BehaviorModule.h"

//-------------------------------------------------------------------------------------------------
/** OBJECT CREATE MODULE base class */
//-------------------------------------------------------------------------------------------------
class CreateModuleInterface
{
public:
	virtual void onCreate() = 0;				///< This is called when you become a code Object
	virtual void onBuildComplete() = 0;	///< This is called when you are a finished game object
	virtual Bool shouldDoOnBuildComplete() const = 0;

};

//-------------------------------------------------------------------------------------------------
class CreateModuleData : public BehaviorModuleData
{
public:

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
		BehaviorModuleData::buildFieldParse(p);
	}
};

//-------------------------------------------------------------------------------------------------
class CreateModule : public BehaviorModule, public CreateModuleInterface
{

	MEMORY_POOL_GLUE_ABC( CreateModule )
	MAKE_STANDARD_MODULE_MACRO_ABC( CreateModule )
	//MAKE_STANDARD_MODULE_DATA_MACRO_ABC(CreateModule, CreateModuleData)

public:

	CreateModule( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	static Int getInterfaceMask() { return MODULEINTERFACE_CREATE; }

	// BehaviorModule
	virtual CreateModuleInterface* getCreate() { return this; }

	virtual void onCreate() = 0;				///< This is called when you become a code Object
	virtual void onBuildComplete(){ m_needToRunOnBuildComplete = FALSE; }	///< This is called when you are a finished game object
	virtual Bool shouldDoOnBuildComplete() const { return m_needToRunOnBuildComplete; }

private:

	Bool m_needToRunOnBuildComplete; ///< Prevent the multiple calling of onBuildComplete

};

#endif
