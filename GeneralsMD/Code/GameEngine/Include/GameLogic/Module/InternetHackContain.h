// FILE: InternetHackContain.cpp //////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:   Contain module that just gives aiHackInternet command to passengers
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __INTERNET_HACK_CONTAIN_H
#define __INTERNET_HACK_CONTAIN_H

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/TransportContain.h"

//-------------------------------------------------------------------------------------------------
class InternetHackContainModuleData : public TransportContainModuleData
{
public:
	

	InternetHackContainModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);
	static void parseRiderInfo( INI* ini, void *instance, void *store, const void* /*userData*/ );

};

//-------------------------------------------------------------------------------------------------
class InternetHackContain : public TransportContain
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( InternetHackContain, "InternetHackContain" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( InternetHackContain, InternetHackContainModuleData )

public:

	InternetHackContain( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onContaining( Object *obj, Bool wasSelected );		///< object now contains 'obj'

protected:

	
private:

};

#endif // __RIDER_CHANGE_CONTAIN_H

