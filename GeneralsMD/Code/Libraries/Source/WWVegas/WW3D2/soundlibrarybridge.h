#ifndef SOUNDLIBRARYBRIDGE_H
#define SOUNDLIBRARYBRIDGE_H

// Forward declarations.
class		Matrix3D;

class SoundLibraryBridgeClass {
	public:
		virtual	void			Play_3D_Audio(const char * name, const Matrix3D & tm) = 0; 
		virtual	void			Play_2D_Audio(const char * name) = 0; 
		virtual	void			Stop_Playing_Audio(const char * name) = 0;
};

#endif //SOUNDLIBRARYBRIDGE_H
