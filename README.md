# Usage

```
-I|--image "path/to/image"
[-g --group <n>]
[-b --blend <blend_mode>]
[-o --output <"path/to/output_image">]
[--multiplier <brightness_multiplier>]
[-D --denoise-iterations <n>]
[--noise-thresh <n>]
[--character-spacing <n>]
```

## Arguments
`image (-I | --image)`: path to the image file to convert, can be a jpg, png or avif file

`group (-g | --group)`: size of pixel groups to turn into characters

`blend (-b | --blend)`: how to determine the brightness of a pixel group, can be the arithmetic average (`avg|average`) the geometric average (`geo|geometric`) the brightest pixel (`brightest|bright|max`) or the darkest pixel (`darkest|dark|min`)

`output (-o | --output)`: where to output the final art, if not specified the art will only be printed to the terminal

`multiplier (--multiplier)`: what to multiply a pixels brightness by for the final product, default is 1.0

`denoise iterations (-D --denoise-iterations)`: how many times the denoise algorithm should pass over the image

`noise threshold (--noise-thresh)`: what the maximum difference from the local mode brightness can be for a pixel before it is denoised

`character spacing (--character-spacing)`: how many pixels to put between 2 characters in the output image
