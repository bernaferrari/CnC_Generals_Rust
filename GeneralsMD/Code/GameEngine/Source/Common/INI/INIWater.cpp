// FILE: INIWater.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   Water settings 
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_TIME_OF_DAY_NAMES

#include "Common/INI.h"
#include "Common/GameType.h"

#include "GameClient/TerrainVisual.h"
#include "GameClient/Water.h"

///////////////////////////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
/** Water setting, note that this does not support override situations.  As the water
	* system becomes more complex we may want to change this */
//-------------------------------------------------------------------------------------------------
void INI::parseWaterSettingDefinition( INI* ini )
{
	AsciiString name;
	WaterSetting *waterSetting = NULL;

	// read the name
	const char* token = ini->getNextToken();

	name.set( token );

	// get the water setting we want to load based on name
	char **timeOfDayName = TimeOfDayNames;
	Int timeOfDayIndex = 0;  // TIME_OF_DAY_INVALID
	while( timeOfDayName && *timeOfDayName )
	{

		if( stricmp( *timeOfDayName, name.str() ) == 0 )
		{

			waterSetting = &WaterSettings[ timeOfDayIndex ];
			break;

		}  // end if

		// next name
		timeOfDayName++;
		timeOfDayIndex++;

	}  // end while

	// check for no time of day match
	if( waterSetting == NULL )
		throw INI_INVALID_DATA;

	// parse the data
	ini->initFromINI( waterSetting, waterSetting->getFieldParse() );

}  // end parseWaterSetting

//-------------------------------------------------------------------------------------------------
void INI::parseWaterTransparencyDefinition( INI *ini )
{
	if (TheWaterTransparency == NULL) {
		TheWaterTransparency = newInstance(WaterTransparencySetting);
	} else if (ini->getLoadType() == INI_LOAD_CREATE_OVERRIDES) {
		WaterTransparencySetting* wt = (WaterTransparencySetting*) (TheWaterTransparency.getNonOverloadedPointer());
		WaterTransparencySetting* wtOverride = newInstance(WaterTransparencySetting);
		*wtOverride = *wt;

		// Mark that it is an override.
		wtOverride->markAsOverride();

		wt->friend_getFinalOverride()->setNextOverride(wtOverride);
	} else {
		throw INI_INVALID_DATA;
	}

	WaterTransparencySetting* waterTrans = (WaterTransparencySetting*) (TheWaterTransparency.getNonOverloadedPointer());
	waterTrans = (WaterTransparencySetting*) (waterTrans->friend_getFinalOverride());
	// parse the data
	ini->initFromINI( waterTrans, TheWaterTransparency->getFieldParse() );
	
	// If we overrode any skybox textures, then call the W3D Water stuff.
	if (ini->getLoadType() == INI_LOAD_CREATE_OVERRIDES) {
		// Check to see if we overrode any skybox textures.
		// If we did, then we need to replace them in the model.
		// Copy/Paste monkeys PLEASE TAKE NOTE. This technique only works for the skybox because we
		// know that there will never be more than one sky box. If you were to use this technique for
		// technicals, for instance, it would make all technicals in the level have the same new
		// texture.

		const WaterTransparencySetting* wtOriginal = TheWaterTransparency.getNonOverloadedPointer();
		OVERRIDE<WaterTransparencySetting> wtOverride = TheWaterTransparency;

		if (wtOriginal == wtOverride) 
			return;

		const AsciiString *oldTextures[5],*newTextures[5];

		//Copy current texture names into arrays
		oldTextures[0]=&wtOriginal->m_skyboxTextureN;
		newTextures[0]=&wtOverride->m_skyboxTextureN;
		oldTextures[1]=&wtOriginal->m_skyboxTextureE;
		newTextures[1]=&wtOverride->m_skyboxTextureE;
		oldTextures[2]=&wtOriginal->m_skyboxTextureS;
		newTextures[2]=&wtOverride->m_skyboxTextureS;
		oldTextures[3]=&wtOriginal->m_skyboxTextureW;
		newTextures[3]=&wtOverride->m_skyboxTextureW;
		oldTextures[4]=&wtOriginal->m_skyboxTextureT;
		newTextures[4]=&wtOverride->m_skyboxTextureT;

		TheTerrainVisual->replaceSkyboxTextures(oldTextures, newTextures);
	}
}


