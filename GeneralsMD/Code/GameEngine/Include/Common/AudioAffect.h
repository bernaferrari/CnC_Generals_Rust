// EA Pacific
// John McDonald, Jr
// Do not distribute
#pragma once

#ifndef _AUDIOAFFECT_H_
#define _AUDIOAFFECT_H_

// if it is set by the options panel, use the system setting parameter. Otherwise, this will be 
// appended to whatever the current system volume is.
enum AudioAffect
{
	AudioAffect_Music		= 0x01,
	AudioAffect_Sound		= 0x02,
	AudioAffect_Sound3D	= 0x04,
	AudioAffect_Speech	= 0x08,
	AudioAffect_All			= (AudioAffect_Music | AudioAffect_Sound | AudioAffect_Sound3D | AudioAffect_Speech),

	AudioAffect_SystemSetting = 0x10,
};

#endif // _AUDIOAFFECT_H_
