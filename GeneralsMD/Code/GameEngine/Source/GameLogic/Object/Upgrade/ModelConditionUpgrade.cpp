// FILE: ModelConditionUpgrade.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, July 2003
// Desc:	 UpgradeModule that sets a modelcondition flag
///////////////////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameLogic/Module/ModelConditionUpgrade.h"

#include "Common/ModelState.h"
#include "GameLogic/Object.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
ModelConditionUpgradeModuleData::ModelConditionUpgradeModuleData()
{
	m_conditionFlag = MODELCONDITION_INVALID;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void ModelConditionUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  UpgradeModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "ConditionFlag",	ModelConditionFlags::parseSingleBitFromINI,	NULL, offsetof( ModelConditionUpgradeModuleData, m_conditionFlag ) },
		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ModelConditionUpgrade::ModelConditionUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ModelConditionUpgrade::~ModelConditionUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ModelConditionUpgrade::upgradeImplementation( )
{
	const ModelConditionUpgradeModuleData *data = getModelConditionUpgradeModuleData();

	Object *me = getObject();	

	if( data->m_conditionFlag != MODELCONDITION_INVALID )
		me->setModelConditionState(data->m_conditionFlag);
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ModelConditionUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void ModelConditionUpgrade::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpgradeModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void ModelConditionUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
