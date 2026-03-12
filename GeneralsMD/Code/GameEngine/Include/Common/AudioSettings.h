// EA Pacific
// John McDonald, Jr
// Do not distribute

#pragma once

#ifndef _AUDIOSETTINGS_H_
#define _AUDIOSETTINGS_H_

#include "Common/AsciiString.h"

enum { MAX_HW_PROVIDERS = 4 };

struct AudioSettings
{
	AsciiString m_audioRoot;
	AsciiString m_soundsFolder;
	AsciiString m_musicFolder;
	AsciiString m_streamingFolder;
	AsciiString m_soundsExtension;
	Bool m_useDigital;
	Bool m_useMidi;
	Int m_outputRate;
	Int m_outputBits;
	Int m_outputChannels;
	Int m_sampleCount2D;
	Int m_sampleCount3D;
	Int m_streamCount;
	Int m_globalMinRange;
	Int m_globalMaxRange;
	Int m_drawableAmbientFrames;
	Int m_fadeAudioFrames;
	UnsignedInt m_maxCacheSize;

	Real m_minVolume;		// At volumes less than this, the sample will be culled.
	
	AsciiString m_preferred3DProvider[MAX_HW_PROVIDERS + 1];

	//Defaults actually don't ever get changed!
	Real m_relative2DVolume;		//2D volume compared to 3D
	Real m_defaultSoundVolume;
	Real m_default3DSoundVolume;
	Real m_defaultSpeechVolume;
	Real m_defaultMusicVolume;
	UnsignedInt m_defaultSpeakerType2D;
	UnsignedInt m_defaultSpeakerType3D;

	//If you want to change a value, store it somewhere else (like here)
	Real m_preferredSoundVolume;
	Real m_preferred3DSoundVolume;
	Real m_preferredSpeechVolume;
	Real m_preferredMusicVolume;

	//The desired altitude of the microphone to improve panning relative to terrain.
	Real m_microphoneDesiredHeightAboveTerrain;

	//When tracing a line between the ground look-at-point and the camera, we want
	//to ensure a maximum percentage, so the microphone never goes behind the camera.
	Real m_microphoneMaxPercentageBetweenGroundAndCamera;

	//Handles changing sound volume whenever the camera is close to the microphone.
  Real m_zoomMinDistance;			//If we're closer than the minimum distance, then apply the full bonus no matter how close.
  Real m_zoomMaxDistance;			//The maximum distance from microphone we need to be before benefiting from any bonus.
	
  //NOTE: The higher this value is, the lower normal sounds will be! If you specify a sound volume value of 25%, then sounds will play
	//between 75% and 100%, not 100% to 125%!
  Real m_zoomSoundVolumePercentageAmount;	//The amount of sound volume dedicated to zooming.
};

#endif // _AUDIOSETTINGS_H_
