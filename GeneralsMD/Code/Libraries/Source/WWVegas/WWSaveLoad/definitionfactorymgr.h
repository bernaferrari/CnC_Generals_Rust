#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __DEFINITION_FACTORY_MGR_H
#define __DEFINITION_FACTORY_MGR_H

#include "always.h"
#include "bittype.h"
#include "definitionclassids.h"

class DefinitionFactoryClass;

//////////////////////////////////////////////////////////////////////////////////
//
//	DefinitionFactoryMgrClass 
//
//////////////////////////////////////////////////////////////////////////////////
class DefinitionFactoryMgrClass
{
public:

	/////////////////////////////////////////////////////////////////////
	//	Public methods
	/////////////////////////////////////////////////////////////////////
	static DefinitionFactoryClass *	Find_Factory (uint32 class_id);	
	static DefinitionFactoryClass *	Find_Factory (const char *name);
	static void								Register_Factory (DefinitionFactoryClass *factory);
	static void								Unregister_Factory (DefinitionFactoryClass *factory);

	// Class enumeration
	static DefinitionFactoryClass *	Get_First (uint32 superclass_id);
	static DefinitionFactoryClass *	Get_Next (DefinitionFactoryClass *current, uint32 superclass_id);

	// Factory enumeration
	static DefinitionFactoryClass *	Get_First (void);
	static DefinitionFactoryClass *	Get_Next (DefinitionFactoryClass *current);
	
private:

	/////////////////////////////////////////////////////////////////////
	//	Private methods
	/////////////////////////////////////////////////////////////////////
	static void								Link_Factory (DefinitionFactoryClass *factory);
	static void								Unlink_Factory (DefinitionFactoryClass *factory);

	/////////////////////////////////////////////////////////////////////
	//	Static member data
	/////////////////////////////////////////////////////////////////////
	static DefinitionFactoryClass *	_FactoryListHead;
};


#endif //__DEFINITION_FACTORY_MGR_H
