#if defined(_MSC_VER)
#pragma once
#endif

#ifndef VECTORPROCESSOR_H
#define VECTORPROCESSOR_H

class Vector2;
class Vector3;
class Vector4;
class Matrix3D;
class Matrix4x4;

class VectorProcessorClass
{
public:
	static void Transform(Vector3* dst,const Vector3 *src, const Matrix3D& matrix, const int count);
	static void Transform(Vector4* dst,const Vector3 *src, const Matrix4x4& matrix, const int count);
	static void Copy(unsigned *dst,const unsigned *src, const int count);
	static void Copy(Vector2 *dst,const Vector2 *src, const int count);
	static void Copy(Vector3 *dst,const Vector3 *src, const int count);
	static void Copy(Vector4 *dst,const Vector4 *src, const int count);
	static void Copy(Vector4 *dst,const Vector3 *src, const float * srca, const int count);
	static void Copy(Vector4 *dst,const Vector3 *src, const float srca, const int count);
	static void Copy(Vector4 *dst,const Vector3 &src, const float * srca, const int count);
	static void CopyIndexed(unsigned *dst,const unsigned *src, const unsigned int *index, const int count);
	static void CopyIndexed(Vector2 *dst,const Vector2 *src, const unsigned int *index, const int count);
	static void CopyIndexed(Vector3 *dst,const Vector3 *src, const unsigned int *index, const int count);
	static void CopyIndexed(Vector4 *dst,const Vector4 *src, const unsigned int *index, const int count);
	static void CopyIndexed(unsigned char* dst, const unsigned char* src, const unsigned int *index, int count);
	static void CopyIndexed(float* dst, float* src, const unsigned int *index, int count);
	static void Clamp(Vector4 *dst,const Vector4 *src, const float min, const float max, const int count);
	static void Clear (Vector3 *dst, const int count);
	static void Normalize(Vector3 *dst, const int count);
	static void MinMax(Vector3 *src, Vector3 &min, Vector3 &max, const int count);

	static void MulAdd(float * dest,float multiplier,float add,int count);

	static void Prefetch(void* address);

	static void DotProduct(float *dst, const Vector3 &a, const Vector3 *b,const int count);
	static void ClampMin(float *dst, float *src, const float min, const int count);
	static void Power(float *dst, float *src, const float pow, const int count);
};

#endif // VECTORPROCESSOR_H
